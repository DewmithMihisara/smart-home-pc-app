mod commands;
pub mod midea;
mod state;
mod tray;

use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_popup(app)))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::connect_device,
            commands::get_config,
            commands::get_state,
            commands::set_state,
            commands::select_device,
            commands::rename_device,
            commands::remove_device,
            commands::quit,
        ])
        .setup(|app| {
            // Menu-bar only app on macOS: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::create(app.handle())?;
            state::start_polling(app.handle().clone());
            // First run: open the popup so the setup wizard is visible.
            if state::load_devices(app.handle()).0.is_empty() {
                tray::show_popup(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                if let Some(w) = window.app_handle().get_webview_window(window.label()) {
                    tray::on_blur(&w);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
