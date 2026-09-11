use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize().ok();
        window.show().ok();
        window.set_focus().ok();
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show", "Open SwitchCraft").build(app)?;
    let quick_switch_item = MenuItemBuilder::with_id("quick_switch", "Quick Switch…").build(app)?;
    let update_item = MenuItemBuilder::with_id("check_update", "Check for Updates").build(app)?;
    let separator = MenuItemBuilder::with_id("sep", "──────────────")
        .enabled(false)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&quick_switch_item)
        .item(&separator)
        .item(&update_item)
        .item(&quit_item)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("SwitchCraft — Session Control")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quick_switch" => {
                show_main_window(app);
                if let Some(window) = app.get_webview_window("main") {
                    window.eval("document.getElementById('btn-quick-switch')?.click()").ok();
                }
            }
            "check_update" => {
                show_main_window(app);
                if let Some(window) = app.get_webview_window("main") {
                    window.eval("document.getElementById('btn-manual-update')?.click()").ok();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            }
            | tauri::tray::TrayIconEvent::DoubleClick {
                button: tauri::tray::MouseButton::Left,
                ..
            } => {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    if is_visible {
                        window.hide().ok();
                    } else {
                        window.unminimize().ok();
                        window.show().ok();
                        window.set_focus().ok();
                    }
                }
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
