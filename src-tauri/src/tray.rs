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
    let quick_switch_item =
        MenuItemBuilder::with_id("quick_switch", "Switch account…").build(app)?;
    let update_item =
        MenuItemBuilder::with_id("check_update", "Check for Updates").build(app)?;
    let separator = MenuItemBuilder::with_id("sep", "──────────────")
        .enabled(false)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit SwitchCraft").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&quick_switch_item)
        .item(&separator)
        .item(&update_item)
        .item(&quit_item)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        // A normal click opens the tray menu. This keeps the main window out of the
        // taskbar while still exposing account switching, update and quit controls.
        .show_menu_on_left_click(true)
        .tooltip("SwitchCraft — Session Control")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quick_switch" => {
                show_main_window(app);
                if let Some(window) = app.get_webview_window("main") {
                    window
                        .eval("document.getElementById('btn-quick-switch')?.click()")
                        .ok();
                }
            }
            "check_update" => {
                show_main_window(app);
                if let Some(window) = app.get_webview_window("main") {
                    window
                        .eval("document.getElementById('btn-manual-update')?.click()")
                        .ok();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        // Double-click remains a fast way to reopen the main window without changing
        // the single-click tray-menu behavior.
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
