//! App state: saved devices, the live LAN session, polling and UI events.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::midea::ac::{AcPatch, AcState};
use crate::midea::lan::{DeviceConfig, LanClient};
use crate::midea::{Error, Result};

const STORE: &str = "config.json";
const POLL_VISIBLE: Duration = Duration::from_secs(10);
const POLL_HIDDEN: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedDevice {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub token: String,
    pub key: String,
}

impl SavedDevice {
    fn config(&self) -> Result<DeviceConfig> {
        let bad = |what: &str| Error::Auth(format!("saved {what} is invalid"));
        let token = hex::decode(&self.token).map_err(|_| bad("token"))?;
        let key: [u8; 32] = hex::decode(&self.key).ok().and_then(|k| k.try_into().ok()).ok_or_else(|| bad("key"))?;
        let id = self.id.parse().map_err(|_| bad("device id"))?;
        Ok(DeviceConfig { ip: self.ip.clone(), port: self.port, id, auth: Some((token, key)) })
    }
}

/// Device list as shown to the UI (no secrets).
#[derive(Clone, Serialize)]
pub struct DeviceSummary {
    id: String,
    name: String,
    ip: String,
}

#[derive(Clone, Serialize)]
pub struct Config {
    devices: Vec<DeviceSummary>,
    active: Option<String>,
}

/// Pushed to the UI on every state change or connection problem.
#[derive(Clone, Serialize)]
pub struct Update {
    status: &'static str, // "online" | "offline" | "unconfigured"
    state: Option<AcState>,
    error: Option<String>,
}

struct Session {
    device_id: String,
    client: LanClient,
    last: Option<AcState>,
}

#[derive(Default)]
pub struct AppState {
    session: Mutex<Option<Session>>,
}

// ---------------------------------------------------------------- persistence

pub fn load_devices<R: Runtime>(app: &AppHandle<R>) -> (Vec<SavedDevice>, Option<String>) {
    let Ok(store) = app.store(STORE) else { return (vec![], None) };
    let devices = store.get("devices").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let active = store.get("active").and_then(|v| v.as_str().map(String::from));
    (devices, active)
}

fn save_devices<R: Runtime>(app: &AppHandle<R>, devices: &[SavedDevice], active: Option<&str>) -> Result<()> {
    let store = app.store(STORE).map_err(|e| Error::Protocol(e.to_string()))?;
    store.set("devices", serde_json::to_value(devices).unwrap_or_default());
    store.set("active", serde_json::json!(active));
    store.save().map_err(|e| Error::Protocol(e.to_string()))
}

pub fn config<R: Runtime>(app: &AppHandle<R>) -> Config {
    let (devices, active) = load_devices(app);
    let active = active.filter(|a| devices.iter().any(|d| &d.id == a)).or_else(|| devices.first().map(|d| d.id.clone()));
    Config {
        devices: devices.into_iter().map(|d| DeviceSummary { id: d.id, name: d.name, ip: d.ip }).collect(),
        active,
    }
}

fn active_device<R: Runtime>(app: &AppHandle<R>) -> Option<SavedDevice> {
    let (devices, active) = load_devices(app);
    active
        .and_then(|a| devices.iter().find(|d| d.id == a).cloned())
        .or_else(|| devices.into_iter().next())
}

/// Add or replace a device and make it the active one.
pub async fn add_device<R: Runtime>(app: &AppHandle<R>, dev: SavedDevice, state: AcState) -> Result<()> {
    let (mut devices, _) = load_devices(app);
    devices.retain(|d| d.id != dev.id);
    devices.push(dev.clone());
    save_devices(app, &devices, Some(&dev.id))?;
    let client = LanClient::new(dev.config()?);
    *app.state::<AppState>().session.lock().await = Some(Session { device_id: dev.id, client, last: Some(state.clone()) });
    emit(app, "online", Some(state), None);
    Ok(())
}

pub async fn select_device<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<()> {
    let (devices, _) = load_devices(app);
    save_devices(app, &devices, Some(id))?;
    *app.state::<AppState>().session.lock().await = None;
    Ok(())
}

pub async fn rename_device<R: Runtime>(app: &AppHandle<R>, id: &str, name: &str) -> Result<()> {
    let (mut devices, active) = load_devices(app);
    if let Some(d) = devices.iter_mut().find(|d| d.id == id) {
        d.name = name.trim().to_string();
    }
    save_devices(app, &devices, active.as_deref())
}

pub async fn remove_device<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<()> {
    let (mut devices, active) = load_devices(app);
    devices.retain(|d| d.id != id);
    let active = active.filter(|a| a != id).or_else(|| devices.first().map(|d| d.id.clone()));
    save_devices(app, &devices, active.as_deref())?;
    let app_state = app.state::<AppState>();
    let mut session = app_state.session.lock().await;
    if session.as_ref().is_some_and(|s| s.device_id == id) {
        *session = None;
    }
    Ok(())
}

// ---------------------------------------------------------------- device access

/// Run `f` against the active device's session, creating it if needed.
async fn with_session<R: Runtime, T>(
    app: &AppHandle<R>,
    f: impl AsyncFnOnce(&mut Session) -> Result<T>,
) -> Result<T> {
    let state = app.state::<AppState>();
    let mut guard = state.session.lock().await;
    let want = active_device(app);
    let Some(dev) = want else {
        *guard = None;
        return Err(Error::Protocol("no air conditioner configured".into()));
    };
    if guard.as_ref().is_none_or(|s| s.device_id != dev.id) {
        *guard = Some(Session { device_id: dev.id.clone(), client: LanClient::new(dev.config()?), last: None });
    }
    f(guard.as_mut().expect("session")).await
}

pub async fn refresh<R: Runtime>(app: &AppHandle<R>) -> Result<AcState> {
    let res = with_session(app, async |s| {
        let st = s.client.query_state().await?;
        s.last = Some(st.clone());
        Ok(st)
    })
    .await;
    report(app, &res);
    res
}

pub async fn apply<R: Runtime>(app: &AppHandle<R>, patch: AcPatch) -> Result<AcState> {
    let res = with_session(app, async |s| {
        let mut st = match s.last.clone() {
            Some(st) => st,
            None => s.client.query_state().await?,
        };
        st.apply(&patch);
        let reported = s.client.set_state(&st).await?;
        let st = reported.unwrap_or(st);
        s.last = Some(st.clone());
        Ok(st)
    })
    .await;
    report(app, &res);
    res
}

pub async fn toggle_power<R: Runtime>(app: &AppHandle<R>) {
    let power = with_session(app, async |s| Ok(s.last.as_ref().map(|st| st.power))).await.ok().flatten();
    let power = match power {
        Some(p) => p,
        None => match refresh(app).await {
            Ok(st) => st.power,
            Err(_) => return,
        },
    };
    let _ = apply(app, AcPatch { power: Some(!power), ..Default::default() }).await;
}

fn report<R: Runtime>(app: &AppHandle<R>, res: &Result<AcState>) {
    match res {
        Ok(st) => emit(app, "online", Some(st.clone()), None),
        Err(e) if active_device(app).is_none() => emit(app, "unconfigured", None, Some(e.to_string())),
        Err(e) => emit(app, "offline", None, Some(e.to_string())),
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, status: &'static str, state: Option<AcState>, error: Option<String>) {
    if let Some(tray) = app.tray_by_id("tray") {
        let tip = match (&state, status) {
            (Some(s), _) if s.power => format!(
                "AC Remote — {:?} {}°C{}",
                s.mode,
                s.target,
                s.indoor.map(|t| format!(" · room {t}°C")).unwrap_or_default()
            ),
            (Some(_), _) => "AC Remote — off".to_string(),
            (None, "offline") => "AC Remote — offline".to_string(),
            _ => "AC Remote".to_string(),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
    let _ = app.emit("ac-update", Update { status, state, error });
}

/// Background polling; faster while the popup is visible.
pub fn start_polling<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            if active_device(&app).is_some() {
                let _ = refresh(&app).await;
            }
            let visible = app
                .get_webview_window(crate::tray::POPUP)
                .and_then(|w| w.is_visible().ok())
                .unwrap_or(false);
            tokio::time::sleep(if visible { POLL_VISIBLE } else { POLL_HIDDEN }).await;
        }
    });
}
