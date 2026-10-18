//! Tauri commands invoked from the UI.

use std::time::Duration;

use serde::Deserialize;
use tauri::AppHandle;

use crate::midea::ac::{AcPatch, AcState};
use crate::midea::cloud::resolve_token;
use crate::midea::discover::{discover, DeviceInfo};
use crate::midea::lan::{DeviceConfig, LanClient};
use crate::midea::{Error, Result};
use crate::state::{self, Config, SavedDevice, Snapshot};

#[tauri::command]
pub async fn discover_devices(ip: Option<String>) -> Result<Vec<DeviceInfo>> {
    let target = match ip.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(s.parse().map_err(|_| Error::Protocol(format!("'{s}' is not a valid IPv4 address")))?),
        None => None,
    };
    let mut devices = discover(target, Duration::from_secs(4)).await?;
    devices.retain(|d| d.device_type == 0xAC || d.device_type == 0);
    devices.sort_by(|a, b| a.ip.cmp(&b.ip));
    Ok(devices)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectArgs {
    ip: String,
    port: u16,
    id: String,
    name: String,
    email: Option<String>,
    password: Option<String>,
    token: Option<String>,
    key: Option<String>,
}

/// Obtain credentials (cloud login or manual token/key), verify them against the device and save it.
#[tauri::command]
pub async fn connect_device(app: AppHandle, args: ConnectArgs) -> Result<AcState> {
    let id: u64 = args.id.parse().map_err(|_| Error::Protocol("invalid device id".into()))?;
    let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let (token, key, state) = match (clean(args.token), clean(args.key)) {
        (Some(token), Some(key)) => {
            let bad = |what: &str| Error::Auth(format!("{what} must be hex"));
            let token_b = hex::decode(&token).map_err(|_| bad("token"))?;
            let key_b: [u8; 32] = hex::decode(&key)
                .ok()
                .and_then(|k| k.try_into().ok())
                .ok_or_else(|| Error::Auth("key must be 64 hex characters".into()))?;
            let mut lan = LanClient::new(DeviceConfig { ip: args.ip.clone(), port: args.port, id, auth: Some((token_b, key_b)) });
            let state = lan.query_state().await?;
            (token, key, state)
        }
        _ => {
            let account = clean(args.email).zip(clean(args.password));
            resolve_token(&args.ip, args.port, id, account).await?
        }
    };

    let dev = SavedDevice { id: args.id, name: args.name.trim().to_string(), ip: args.ip, port: args.port, token, key };
    state::add_device(&app, dev, state.clone()).await?;
    Ok(state)
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Config {
    state::config(&app)
}

#[tauri::command]
pub async fn get_state(app: AppHandle) -> Result<Snapshot> {
    state::refresh(&app).await
}

#[tauri::command]
pub async fn set_state(app: AppHandle, patch: AcPatch) -> Result<Snapshot> {
    state::apply(&app, patch).await
}

#[tauri::command]
pub async fn select_device(app: AppHandle, id: String) -> Result<()> {
    state::select_device(&app, &id).await
}

#[tauri::command]
pub async fn rename_device(app: AppHandle, id: String, name: String) -> Result<()> {
    state::rename_device(&app, &id, &name).await
}

#[tauri::command]
pub async fn remove_device(app: AppHandle, id: String) -> Result<()> {
    state::remove_device(&app, &id).await
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}
