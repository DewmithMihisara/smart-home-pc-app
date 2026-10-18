//! Minimal Midea cloud client — only used once per device to fetch its V3 token/key.
//!
//! Two clouds are supported (both used by msmart-ng): NetHome Plus and SmartHome.
//! Built-in shared accounts work for token lookup; a personal account is optional.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::Sha256;

use super::{Error, Result};

fn cloud_err(msg: impl Into<String>) -> Error {
    Error::Cloud(msg.into())
}

fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn md5_hex(s: &str) -> String {
    hex::encode(Md5::digest(s.as_bytes()))
}

/// UTC timestamp formatted as `YYYYMMDDHHMMSS`.
fn stamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!("{y:04}{m:02}{d:02}{:02}{:02}{:02}", t / 3600, t / 60 % 60, t % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| cloud_err(e.to_string()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudKind {
    NetHomePlus,
    SmartHome,
}

pub struct Cloud {
    kind: CloudKind,
    client: reqwest::Client,
    account: String,
    password: String,
    device_id: String,
    session_id: String,
    access_token: String,
}

impl Cloud {
    /// Use the given account, or the built-in shared account for that cloud.
    pub fn new(kind: CloudKind, account: Option<(String, String)>) -> Result<Self> {
        let (account, password) = account.unwrap_or_else(|| {
            let (a, p) = match kind {
                CloudKind::NetHomePlus => ("nethome+us@mailinator.com", "password1"),
                CloudKind::SmartHome => ("midea@mailinator.com", "this_is_a_password1"),
            };
            (a.to_string(), p.to_string())
        });
        Ok(Self {
            kind,
            client: http()?,
            account,
            password,
            device_id: random_hex(8),
            session_id: String::new(),
            access_token: String::new(),
        })
    }

    pub async fn login(&mut self) -> Result<()> {
        let login_id = self
            .api("/v1/user/login/id/get", self.body([("loginAccount", json!(self.account))]))
            .await?["loginId"]
            .as_str()
            .ok_or_else(|| cloud_err("missing loginId"))?
            .to_string();

        match self.kind {
            CloudKind::NetHomePlus => {
                let pwd = nhp::encrypt_password(&login_id, &self.password);
                let res = self
                    .api("/v1/user/login", self.body([("loginAccount", json!(self.account)), ("password", json!(pwd))]))
                    .await?;
                self.session_id = res["sessionId"].as_str().ok_or_else(|| cloud_err("missing sessionId"))?.to_string();
            }
            CloudKind::SmartHome => {
                let body = json!({
                    "data": { "platform": 2, "deviceId": self.device_id },
                    "iotData": {
                        "appId": sh::APP_ID,
                        "src": sh::APP_ID,
                        "clientType": 1,
                        "loginAccount": self.account,
                        "iampwd": sh::encrypt_iam_password(&login_id, &self.password),
                        "password": sh::encrypt_password(&login_id, &self.password),
                        "pushToken": random_hex(90),
                        "stamp": stamp(),
                        "reqId": random_hex(16),
                    }
                });
                let res = self.api("/mj/user/login", body.as_object().unwrap().clone().into_iter().collect()).await?;
                self.access_token = res["mdata"]["accessToken"]
                    .as_str()
                    .ok_or_else(|| cloud_err("missing accessToken"))?
                    .to_string();
            }
        }
        Ok(())
    }

    /// Fetch the (token, key) pair for a device's udpid.
    pub async fn get_token(&self, udpid: &str, device_id: u64) -> Result<(String, String)> {
        // SmartHome rejects getToken unless the appliance id is included; other clouds ignore it.
        let body = self.body([("udpid", json!(udpid)), ("applianceCodes", json!(device_id.to_string()))]);
        let res = self.api("/v1/iot/secure/getToken", body).await?;
        res["tokenlist"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["udpId"].as_str() == Some(udpid))
            .and_then(|t| Some((t["token"].as_str()?.to_string(), t["key"].as_str()?.to_string())))
            .ok_or_else(|| cloud_err(format!("no token found for udpid {udpid}")))
    }

    fn body<const N: usize>(&self, extra: [(&str, Value); N]) -> BTreeMap<String, Value> {
        let app_id = match self.kind {
            CloudKind::NetHomePlus => nhp::APP_ID,
            CloudKind::SmartHome => sh::APP_ID,
        };
        let mut b = BTreeMap::new();
        b.insert("appId".into(), json!(app_id));
        b.insert("src".into(), json!(app_id));
        b.insert("format".into(), json!(2));
        b.insert("clientType".into(), json!(1));
        b.insert("language".into(), json!("en_US"));
        b.insert("deviceId".into(), json!(self.device_id));
        b.insert("stamp".into(), json!(stamp()));
        match self.kind {
            CloudKind::NetHomePlus => b.insert("sessionId".into(), json!(self.session_id)),
            CloudKind::SmartHome => b.insert("reqId".into(), json!(random_hex(16))),
        };
        for (k, v) in extra {
            b.insert(k.into(), v);
        }
        b
    }

    async fn api(&self, endpoint: &str, body: BTreeMap<String, Value>) -> Result<Value> {
        let resp = match self.kind {
            CloudKind::NetHomePlus => {
                let mut form: Vec<(String, String)> = body.into_iter().map(|(k, v)| (k, value_str(&v))).collect();
                form.push(("sign".into(), nhp::sign(endpoint, &form)));
                self.client.post(format!("{}{endpoint}", nhp::BASE_URL)).form(&form).send().await
            }
            CloudKind::SmartHome => {
                let contents = serde_json::to_string(&body).map_err(|e| cloud_err(e.to_string()))?;
                let random = random_hex(16);
                self.client
                    .post(format!("{}/mas/v5/app/proxy?alias={endpoint}", sh::BASE_URL))
                    .header("Content-Type", "application/json")
                    .header("secretVersion", "1")
                    .header("sign", sh::sign(&contents, &random))
                    .header("random", random)
                    .header("accessToken", &self.access_token)
                    .body(contents)
                    .send()
                    .await
            }
        };
        let resp = resp.map_err(|e| cloud_err(format!("request failed: {e}")))?;
        let json: Value = resp.json().await.map_err(|e| cloud_err(format!("bad response: {e}")))?;

        let (code_key, data_key) = match self.kind {
            CloudKind::NetHomePlus => ("errorCode", "result"),
            CloudKind::SmartHome => ("code", "data"),
        };
        let code = match &json[code_key] {
            Value::String(s) => s.parse::<i64>().unwrap_or(-1),
            v => v.as_i64().unwrap_or(-1),
        };
        if code != 0 {
            return Err(cloud_err(format!("{} (code {code})", json["msg"].as_str().unwrap_or("unknown error"))));
        }
        Ok(json[data_key].clone())
    }
}

fn value_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

mod nhp {
    use super::sha256_hex;

    pub const APP_ID: &str = "1017";
    pub const BASE_URL: &str = "https://mapp.appsmb.com";
    const APP_KEY: &str = "3742e9e5842d4ad59c2db887e12449f9";

    /// SHA256 of path + sorted `k=v&...` query + app key.
    pub fn sign(path: &str, form: &[(String, String)]) -> String {
        let mut pairs: Vec<_> = form.iter().collect();
        pairs.sort();
        let query = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
        sha256_hex(&format!("{path}{query}{APP_KEY}"))
    }

    pub fn encrypt_password(login_id: &str, password: &str) -> String {
        sha256_hex(&format!("{login_id}{}{APP_KEY}", sha256_hex(password)))
    }
}

mod sh {
    use super::{md5_hex, sha256_hex, Hmac, Mac, Sha256};

    pub const APP_ID: &str = "1010";
    pub const BASE_URL: &str = "https://mp-prod.appsmb.com";
    const HMAC_KEY: &str = "PROD_VnoClJI9aikS8dyy";
    const IOT_KEY: &str = "meicloud";
    const LOGIN_KEY: &str = "ac21b9f9cbfe4ca5a88562ef25e2b768";

    pub fn sign(contents: &str, random: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(HMAC_KEY.as_bytes()).expect("any key length");
        mac.update(format!("{IOT_KEY}{contents}{random}").as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub fn encrypt_password(login_id: &str, password: &str) -> String {
        sha256_hex(&format!("{login_id}{}{LOGIN_KEY}", sha256_hex(password)))
    }

    pub fn encrypt_iam_password(login_id: &str, password: &str) -> String {
        let m2 = md5_hex(&md5_hex(password));
        sha256_hex(&format!("{login_id}{m2}{LOGIN_KEY}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nhp_sign_sorts_fields() {
        let a = nhp::sign("/v1/x", &[("b".into(), "2".into()), ("a".into(), "1".into())]);
        let b = sha256_hex("/v1/xa=1&b=23742e9e5842d4ad59c2db887e12449f9");
        assert_eq!(a, b);
    }

    #[test]
    fn stamp_format() {
        let s = stamp();
        assert_eq!(s.len(), 14);
        assert!(s.starts_with("20"));
    }
}

/// Find a working token/key for a device: try each cloud and both udpid byte orders,
/// and keep the first pair the device actually accepts.
pub async fn resolve_token(
    ip: &str,
    port: u16,
    device_id: u64,
    account: Option<(String, String)>,
) -> Result<(String, String, crate::midea::ac::AcState)> {
    use super::discover::udpid;
    use super::lan::{DeviceConfig, LanClient};

    // A personal account is almost always a SmartHome (MSmartHome) one; the shared
    // accounts can only see devices they own, so they are a best-effort fallback.
    let order = if account.is_some() {
        [CloudKind::SmartHome, CloudKind::NetHomePlus]
    } else {
        [CloudKind::NetHomePlus, CloudKind::SmartHome]
    };
    let mut last_err = cloud_err("no cloud available");
    for kind in order {
        let mut cloud = match Cloud::new(kind, account.clone()) {
            Ok(c) => c,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        if let Err(e) = cloud.login().await {
            log::warn!("{kind:?} login failed: {e}");
            last_err = e;
            continue;
        }
        for little_endian in [true, false] {
            let id = udpid(device_id, little_endian);
            let (token, key) = match cloud.get_token(&id, device_id).await {
                Ok(t) => t,
                Err(e) => {
                    log::info!("{kind:?} getToken failed: {e}");
                    last_err = e;
                    continue;
                }
            };
            let (Ok(token_b), Ok(key_b)) = (hex::decode(&token), hex::decode(&key)) else { continue };
            let Ok(key_arr) = <[u8; 32]>::try_from(key_b) else { continue };
            let mut lan = LanClient::new(DeviceConfig { ip: ip.into(), port, id: device_id, auth: Some((token_b, key_arr)) });
            match lan.query_state().await {
                Ok(state) => return Ok((token, key, state)),
                Err(e) => {
                    log::info!("{kind:?} token ({}) rejected: {e}", if little_endian { "LE" } else { "BE" });
                    last_err = e;
                }
            }
        }
    }
    Err(last_err)
}
