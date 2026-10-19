use std::sync::Arc;
use tauri::{
    menu::{AboutMetadata, MenuBuilder, MenuItem, SubmenuBuilder},
    AppHandle, Manager, Runtime,
};

pub fn create_main_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let pkg_info = app.package_info();

    // Notion Lite Menu
    let notion_menu = SubmenuBuilder::new(app, "Notion Lite")
        .about(Some(AboutMetadata {
            name: Some("Notion Lite".to_string()),

            version: Some(pkg_info.version.to_string()),
            ..Default::default()
        }))
        .separator()
        .item(&MenuItem::with_id(
            app,
            "check_updates",
            "Check for Updates...",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "preferences",
            "Preferences",
            true,
            Some("CmdOrCtrl+,"),
        )?)
        .separator()
        .quit()
        .build()?;

    // Navigation Menu
    let nav_menu = SubmenuBuilder::new(app, "Navigation")
        .item(&MenuItem::with_id(
            app,
            "nav_back",
            "Back",
            true,
            Some("Alt+Left"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "nav_forward",
            "Forward",
            true,
            Some("Alt+Right"),
        )?)
        .item(&MenuItem::with_id(
            app,
            "nav_refresh",
            "Refresh",
            true,
            Some("CmdOrCtrl+R"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            "nav_home",
            "Home",
            true,
            Some("CmdOrCtrl+H"),
        )?)
        .build()?;

    // Edit Menu
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // Theme Submenu
    let theme_submenu = SubmenuBuilder::new(app, "Theme")
        .item(&MenuItem::with_id(
            app,
            "theme_light",
            "Light (Default)",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "theme_dracula",
            "Dracula",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "theme_nord",
            "Nord",
            true,
            None::<&str>,
        )?)
        .build()?;

    // View Menu
    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&MenuItem::with_id(
            app,
            "reload",
            "Reload",
            true,
            Some("F5"),
        )?)
        .separator()
        .item(&theme_submenu)
        .separator()
        .item(&MenuItem::with_id(
            app,
            "toggle_dev_tools",
            "Toggle Developer Tools",
            true,
            Some("F12"),
        )?)
        .separator()
        .item(&MenuItem::with_id(
            app,
            "toggle_menu_bar",
            "Toggle Menu Bar",
            true,
            Some("CmdOrCtrl+Shift+M"),
        )?)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&notion_menu)
        .item(&nav_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .build()?;

    app.set_menu(menu)?;

    app.on_menu_event(move |app_handle, event| {
        match event.id().as_ref() {
            "nav_back" => {
                log::info!("Menu: Back");
                if let Some(tab_id) = get_active_tab_id(app_handle) {
                    if let Some(wv) = app_handle.get_webview(&tab_id) {
                        let _ = wv.eval("window.history.back()");
                    }
                }
            }
            "nav_forward" => {
                log::info!("Menu: Forward");
                if let Some(tab_id) = get_active_tab_id(app_handle) {
                    if let Some(wv) = app_handle.get_webview(&tab_id) {
                        let _ = wv.eval("window.history.forward()");
                    }
                }
            }
            "nav_refresh" | "reload" => {
                log::info!("Menu: Refresh");
                if let Some(tab_id) = get_active_tab_id(app_handle) {
                    if let Some(wv) = app_handle.get_webview(&tab_id) {
                        let _ = wv.eval("window.location.reload()");
                    }
                }
            }
            "nav_home" => {
                log::info!("Menu: Home");
                let home_url =
                    if let Some(config) = app_handle.try_state::<crate::config::LotionConfig>() {
                        config.get_start_url().to_string()
                    } else {
                        "https://app.notion.com".to_string()
                    };
                if let Some(tab_id) = get_active_tab_id(app_handle) {
                    if let Some(wv) = app_handle.get_webview(&tab_id) {
                        let _ = wv.eval(format!("window.location.href = '{}'", home_url));
                    }
                }
            }
            "preferences" => {
                log::info!("Menu: Preferences");
                use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                use tauri_plugin_shell::ShellExt;

                let active_theme = if let Some(theming) =
                    app_handle.try_state::<Arc<dyn crate::traits::ThemingEngine<R>>>()
                {
                    theming.get_active_theme()
                } else {
                    "default".to_string()
                };

                let config_dir = crate::config::NotionConfig::config_dir();
                let config_path = crate::config::NotionConfig::config_path();
                let menu_visible = if let Some(w) = app_handle.get_window("main") {
                    w.is_menu_visible().unwrap_or(true)
                } else {
                    true
                };

                let message = format!(
                    "Notion Lite Preferences\n\n\
                    • Theme: {}\n\
                    • Menu Bar Visible: {}\n\
                    • Config Directory: {}\n\
                    • Config File: {}\n\n\
                    Keyboard Shortcuts:\n\
                    • Ctrl+Shift+M: Toggle Menu Bar Visibility\n\
                    • Ctrl+,: Open Preferences\n\
                    • Alt+Left / Alt+Right: Back / Forward\n\
                    • Ctrl+R / F5: Refresh\n\
                    • Ctrl+H: Home (Notion.so)\n\
                    • F12: Toggle Developer Tools\n\n\
                    Click OK to open the configuration directory.",
                    active_theme,
                    if menu_visible { "Yes" } else { "No" },
                    config_dir.display(),
                    config_path.display()
                );

                let handle_clone = app_handle.clone();
                let config_dir_str = config_dir.to_string_lossy().to_string();
                let config_file_str = config_path.to_string_lossy().to_string();

                let mut dialog_builder = app_handle
                    .dialog()
                    .message(message)
                    .title("Preferences")
                    .kind(MessageDialogKind::Info);

                if let Some(w) = app_handle.get_window("main") {
                    dialog_builder = dialog_builder.parent(&w);
                }

                dialog_builder.show(move |confirmed| {
                    if confirmed {
                        #[allow(deprecated)]
                        if handle_clone.shell().open(&config_file_str, None).is_err() {
                            let _ = handle_clone.shell().open(&config_dir_str, None);
                        }
                    }
                });
            }
            "toggle_menu_bar" => {
                log::info!("Menu: Toggle Menu Bar");
                if let Some(w) = app_handle.get_window("main") {
                    match w.is_menu_visible() {
                        Ok(true) => {
                            let _ = w.hide_menu();
                        }
                        Ok(false) => {
                            let _ = w.show_menu();
                        }
                        Err(e) => {
                            log::warn!("Failed to check menu visibility: {}", e);
                        }
                    }
                }
            }
            "toggle_dev_tools" => {
                log::info!("Menu: Toggle Developer Tools");
                #[cfg(feature = "devtools")]
                {
                    if let Some(tab_id) = get_active_tab_id(app_handle) {
                        if let Some(wv) = app_handle.get_webview(&tab_id) {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                    }
                }
                #[cfg(not(feature = "devtools"))]
                {
                    log::info!("Developer tools are disabled in this build");
                }
            }
            "check_updates" => {
                log::info!("Menu: Check for Updates requested");
                let handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_updater::UpdaterExt;
                    match handle.updater() {
                        Ok(updater) => match updater.check().await {
                            Ok(Some(update)) => {
                                log::info!("Update available: v{}", update.version);
                                let mut downloaded: usize = 0;
                                let _ = update
                                    .download_and_install(
                                        |chunk_length, content_length| {
                                            downloaded += chunk_length;
                                            log::info!(
                                                "Downloaded {} of {:?}",
                                                downloaded,
                                                content_length
                                            );
                                        },
                                        || {
                                            log::info!("Update download complete, installing...");
                                        },
                                    )
                                    .await;
                                log::info!("Update installed. Restarting...");
                                handle.restart();
                            }
                            Ok(None) => {
                                log::info!("App is up to date.");
                            }
                            Err(e) => {
                                log::warn!("Update check failed: {}", e);
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to get updater: {}", e);
                        }
                    }
                });
            }
            theme_id if theme_id.starts_with("theme_") => {
                let theme_name = theme_id.replace("theme_", "");
                log::info!("Menu: Switch theme to {}", theme_name);

                // Retrieve state manually to avoid borrowing issues
                if let Some(theming) =
                    app_handle.try_state::<Arc<dyn crate::traits::ThemingEngine<R>>>()
                {
                    theming.set_active_theme(&theme_name);

                    if let Some(orchestrator) =
                        app_handle.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>()
                    {
                        let tab_ids = orchestrator.get_tab_ids();
                        for tab_id in tab_ids {
                            let _ = orchestrator.inject_theme_into_tab(
                                app_handle,
                                &tab_id,
                                &theme_name,
                            );
                        }
                    }

                    // Persist to config
                    if let Some(config_state) =
                        app_handle.try_state::<crate::config::LotionConfig>()
                    {
                        let mut config = config_state.inner().clone();
                        config.active_theme = theme_name;
                        let _ = config.save();
                    }
                }
            }
            _ => {}
        }
    });

    Ok(())
}

fn get_active_tab_id<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    if let Some(state_lock) = app.try_state::<Arc<tokio::sync::Mutex<crate::state::AppState>>>() {
        let app_state = state_lock.blocking_lock();
        if let Some(w) = app_state.windows.get("main") {
            return w
                .active_tab_id
                .clone()
                .or_else(|| w.tab_ids.last().cloned());
        }
    }
    None
}
