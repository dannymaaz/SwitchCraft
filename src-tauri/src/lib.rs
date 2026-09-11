mod accounts;
mod tray;

use tauri::Manager;

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    window.hide().ok();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            hide_window,
            accounts::get_accounts,
            accounts::add_account,
            accounts::delete_account,
            accounts::switch_account,
            accounts::import_current_account,
            accounts::update_usage,
            accounts::get_active_accounts,
            accounts::rename_account,
        ])
        .setup(|app| {
            // Setup system tray
            tray::setup_tray(app.handle())?;

            // Hide window on close (minimize to tray instead)
            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    window_clone.hide().ok();
                }
            });

            // If launched with --hidden flag, keep window hidden (autostart behavior)
            let args: Vec<String> = std::env::args().collect();
            if !args.contains(&"--hidden".to_string()) {
                let w = app.get_webview_window("main").unwrap();
                w.show().ok();
                w.center().ok();
                w.set_focus().ok();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SwitchCraft");
}
