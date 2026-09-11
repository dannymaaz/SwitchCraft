mod accounts;
mod atomic_fs;
mod providers;
mod secure_store;
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

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    window.minimize().ok();
}

#[tauri::command]
fn open_browser_url(url: String) -> Result<(), String> {
    const ALLOWED_URLS: [&str; 2] = [
        "https://github.com/dannymaaz/SwitchCraft",
        "https://github.com/dannymaaz/SwitchCraft/releases/latest",
    ];

    if !ALLOWED_URLS.iter().any(|allowed| *allowed == url) {
        return Err("This URL is not in SwitchCraft's allowlist".into());
    }

    open::that(&url).map_err(|e| format!("Failed to open URL: {e}"))
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
            minimize_window,
            open_browser_url,
            accounts::get_accounts,
            accounts::add_account,
            accounts::delete_account,
            accounts::switch_account,
            accounts::import_current_account,
            accounts::restore_last_session,
            accounts::update_usage,
            accounts::get_active_accounts,
            accounts::rename_account,
            accounts::get_security_summary,
        ])
        .setup(|app| {
            tray::setup_tray(app.handle())?;

            let window = app.get_webview_window("main").unwrap();
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    window_clone.hide().ok();
                }
            });

            let args: Vec<String> = std::env::args().collect();
            if !args.contains(&"--hidden".to_string()) {
                let window = app.get_webview_window("main").unwrap();
                window.show().ok();
                window.center().ok();
                window.set_focus().ok();
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running SwitchCraft");
}
