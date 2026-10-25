use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

/// Builds the system tray icon and its menu.
///
/// Linux StatusNotifierItem/AppIndicator requires a real tray menu, so this
/// runs on every platform rather than only where a native indicator exists.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(icon) = app.default_window_icon() else {
        log::warn!("Default window icon not found; system tray not spawned.");
        return Ok(());
    };

    let open_item = MenuItem::with_id(app, "tray_open", "Open", true, None::<&str>)?;
    let check_updates_item = MenuItem::with_id(
        app,
        "tray_check_updates",
        "Check for Updates...",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "Quit", true, None::<&str>)?;

    let tray_menu = MenuBuilder::new(app)
        .item(&open_item)
        .item(&check_updates_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let tray_builder = TrayIconBuilder::with_id("main-tray")
        .icon(icon.clone())
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_open" => show_main_window(app),
            "tray_check_updates" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::updater::check_for_updates(handle).await {
                        log::warn!("Tray: Update check failed: {}", e);
                    }
                });
            }
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    tray_builder.build(app)?;
    log::info!("System tray icon initialized successfully.");
    Ok(())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
