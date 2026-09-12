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
    let update_item = MenuItemBuilder::with_id("check_update", "Check for Updates").build(app)?;
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

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("SwitchCraft default application icon is unavailable")?;

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        // Windows and macOS can show this menu on a normal left click. Tauri's Linux
        // AppIndicator backend does not support controlling the left-click gesture, but
        // the same menu remains available through the desktop environment's context menu.
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
        // Tray click events are available on Windows/macOS. Linux AppIndicator does not
        // emit them, so Linux users reopen the window through the tray context menu.
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
