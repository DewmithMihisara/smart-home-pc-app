//! System tray icon and the popup window anchored to it.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Rect, Runtime, WebviewWindow};

pub const POPUP: &str = "main";
const MARGIN: f64 = 8.0;

/// Remembers where the tray icon is and when the popup was last hidden.
#[derive(Default)]
pub struct TrayState {
    rect: Mutex<Option<(PhysicalPosition<f64>, PhysicalSize<f64>)>>,
    hidden_at: Mutex<Option<Instant>>,
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.manage(TrayState::default());

    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let power = MenuItem::with_id(app, "power", "Toggle power", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit AC Remote", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &power, &PredefinedMenuItem::separator(app)?, &quit])?;

    let icon = app.default_window_icon().cloned().expect("app icon missing");
    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .tooltip("AC Remote")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_popup(app),
            "power" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { crate::state::toggle_power(&app).await });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { rect, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                remember_rect(app, rect);
                toggle_popup(app);
            }
        })
        .build(app)?;
    Ok(())
}

fn remember_rect<R: Runtime>(app: &AppHandle<R>, rect: Rect) {
    let scale = app
        .get_webview_window(POPUP)
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    *app.state::<TrayState>().rect.lock().unwrap() = Some((pos, size));
}

/// Called when the popup loses focus.
pub fn on_blur<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.hide();
    *window.state::<TrayState>().hidden_at.lock().unwrap() = Some(Instant::now());
}

fn toggle_popup<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(POPUP) else { return };
    // Clicking the tray icon while the popup is open blurs (and hides) it first;
    // don't immediately re-open it in that case.
    let just_hidden = app
        .state::<TrayState>()
        .hidden_at
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(250));
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else if !just_hidden {
        show_popup(app);
    }
}

pub fn show_popup<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(POPUP) else { return };
    if let Some(pos) = popup_position(app, &window) {
        let _ = window.set_position(pos);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

/// Place the popup next to the tray icon, clamped to the monitor's work area.
fn popup_position<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) -> Option<PhysicalPosition<i32>> {
    let size = window.outer_size().ok()?;
    let (w, h) = (size.width as f64, size.height as f64);
    let tray = *app.state::<TrayState>().rect.lock().unwrap();

    let monitor = match tray {
        Some((p, _)) => window.monitor_from_point(p.x, p.y).ok().flatten(),
        None => None,
    }
    .or_else(|| window.primary_monitor().ok().flatten())?;
    let area = monitor.work_area();
    let (ax, ay) = (area.position.x as f64, area.position.y as f64);
    let (aw, ah) = (area.size.width as f64, area.size.height as f64);
    let margin = MARGIN * monitor.scale_factor();

    let (x, y) = match tray {
        Some((p, s)) => {
            let x = p.x + s.width / 2.0 - w / 2.0;
            // Taskbar/menu bar at the top (macOS): open below; otherwise open above.
            let y = if p.y < ay + ah / 2.0 { p.y + s.height + margin } else { p.y - h - margin };
            (x, y)
        }
        None => (ax + aw - w - margin, ay + ah - h - margin),
    };
    let x = x.clamp(ax + margin, ax + aw - w - margin);
    let y = y.clamp(ay + margin, ay + ah - h - margin);
    Some(PhysicalPosition::new(x.round() as i32, y.round() as i32))
}
