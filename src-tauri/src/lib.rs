mod accounts;
mod atomic_fs;
mod legacy_cleanup;
mod providers;
mod secure_store;
mod tray;

use tauri::Manager;

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn get_install_channel() -> String {
    #[cfg(target_os = "windows")]
    {
        let exe = match std::env::current_exe() {
            Ok(path) => path.to_string_lossy().to_lowercase(),
            Err(_) => return "unknown".into(),
        };

        let in_env_path = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| exe.starts_with(&value.to_lowercase()))
                .unwrap_or(false)
        };

        // Tauri's WiX MSI installs under Program Files by default. SwitchCraft's
        // configured NSIS installer is current-user and installs under LOCALAPPDATA.
        // Keep these channels distinct so an MSI installation is never silently
        // converted into an NSIS registration by the automatic updater.
        if in_env_path("ProgramFiles") || in_env_path("ProgramFiles(x86)") {
            return "msi".into();
        }
        if in_env_path("LOCALAPPDATA") {
            return "nsis".into();
        }
        return "unknown".into();
    }

    #[cfg(target_os = "macos")]
    {
        "macos".into()
    }

    #[cfg(target_os = "linux")]
    {
        "linux".into()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown".into()
    }
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    window.hide().ok();
}

#[tauri::command]
fn minimize_window(window: tauri::Window) {
    // SwitchCraft is a tray-first app: the titlebar minimize action should remove
    // the window from the taskbar while keeping the process and tray icon alive.
    window.hide().ok();
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
        .on_page_load(|_webview, _| {
            #[cfg(target_os = "macos")]
            {
                // SwitchCraft uses a custom titlebar, so reproduce macOS window-control
                // placement explicitly: close/minimize on the left, app identity next to
                // them, and Quick Switch on the far right. Windows/Linux keep the existing
                // right-side controls.
                let _ = _webview.eval(
                    r#"
                    (() => {
                      const applyMacTitlebar = () => {
                        const titlebar = document.getElementById('titlebar');
                        const brand = titlebar?.querySelector('.titlebar-brand');
                        const right = titlebar?.querySelector('.titlebar-right');
                        const controls = right?.querySelector('.window-controls') || titlebar?.querySelector('.window-controls');
                        if (!titlebar || !brand || !right || !controls) return;

                        if (controls.parentElement !== titlebar) {
                          titlebar.insertBefore(controls, brand);
                        }
                        controls.style.flexDirection = 'row-reverse';
                        titlebar.style.justifyContent = 'flex-start';
                        titlebar.style.gap = '8px';
                        right.style.marginLeft = 'auto';
                      };

                      if (document.readyState === 'loading') {
                        document.addEventListener('DOMContentLoaded', applyMacTitlebar, { once: true });
                      } else {
                        applyMacTitlebar();
                      }
                    })();
                    "#,
                );
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            get_install_channel,
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
            // SwitchCraft v1.0.1 created plaintext credential copies under
            // ~/.switchcraft/backups. Remove only files matching that exact generated
            // pattern before the application becomes available. A cleanup failure is
            // treated as a startup error so we never silently claim a secure migration
            // while known legacy credential copies remain on disk.
            legacy_cleanup::cleanup_v1_plaintext_backups()?;

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
