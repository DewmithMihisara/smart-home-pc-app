//! Live test: nudge the target by -1°C, show the device's reply and a fresh query, then restore.
use std::time::Duration;
use ac_remote_lib::midea::{ac::{self, AcPatch}, lan::{DeviceConfig, LanClient}};

#[tokio::main]
async fn main() {
    let path = std::path::Path::new(&std::env::var("APPDATA").unwrap_or_default()).join("com.acremote.app/config.json");
    let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let d = &cfg["devices"][0];
    let token = hex::decode(d["token"].as_str().unwrap()).unwrap();
    let key: [u8; 32] = hex::decode(d["key"].as_str().unwrap()).unwrap().try_into().unwrap();
    let mut c = LanClient::new(DeviceConfig { ip: d["ip"].as_str().unwrap().into(), port: d["port"].as_u64().unwrap() as u16, id: d["id"].as_str().unwrap().parse().unwrap(), auth: Some((token, key)) });
    let show = |label: &str, frames: &Vec<Vec<u8>>| for f in frames {
        println!("{label}: {} -> {:?}", hex::encode(f), ac::parse_state(f).ok().flatten().map(|s| s.target));
    };

    let orig = c.query_state().await.unwrap();
    println!("before: target {}", orig.target);
    let mut st = orig.clone();
    st.apply(&AcPatch { target: Some(orig.target - 1.0), ..Default::default() });
    show("reply to set", &c.raw(&ac::set_state(&st, false)).await.unwrap());
    tokio::time::sleep(Duration::from_secs(2)).await;
    show("query +2s", &c.raw(&ac::query_state()).await.unwrap());
    show("restore reply", &c.raw(&ac::set_state(&orig, false)).await.unwrap());
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("after restore: target {}", c.query_state().await.unwrap().target);
}
