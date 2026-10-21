use notion::policy::PolicyManager;
use notion::theming::ThemeManager;
use notion_lite as notion;

use notion::config::NotionConfig;
use notion::i18n::I18nManager;
use notion::spellcheck::SpellcheckManager;
use notion::state::AppState;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("NO_AT_BRIDGE", "1");

        // WebKitGTK DMABUF renderer & compositing mode are known to cause severe text corruption,
        // blurry fonts, sticky phantom webviews, and Wayland subsurface artifacts on Linux.
        // Disabling DMABUF renderer & compositing mode makes WebKitGTK render text with native
        // Cairo subpixel antialiasing and allows GTK widget hide/show to function reliably.
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }

        // Optional hardware acceleration fallback for problematic GPU drivers
        if std::env::var("NOTION_DISABLE_GPU")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            log::info!("Hardware acceleration disabled by NOTION_DISABLE_GPU");
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    // Set RUST_LOG only if not already set by the user
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    log::info!("Starting Notion...");

    // Get or create application secret
    let app_secret =
        notion::secret::get_or_create().expect("Failed to get or create application secret");
    let app_secret_arc = Arc::new(app_secret);

    // Load user config
    let config = NotionConfig::load();
    log::info!(
        "Config: theme={}, restore_tabs={}, show_menu_bar={}",
        config.active_theme,
        config.restore_tabs,
        config.show_menu_bar
    );
    let config_arc = Arc::new(std::sync::RwLock::new(config));

    // Load saved state (if any)
    let app_state = AppState::load_from_disk(&app_secret_arc).unwrap_or_default();
    let app_state = Arc::new(tokio::sync::Mutex::new(app_state));

    // Initialize Concrete Modules
    notion::security::init();
    let policy = Arc::new(PolicyManager::new());
    let active_theme = {
        config_arc
            .read()
            .map(|c| c.active_theme.clone())
            .unwrap_or_else(|_| "dracula".to_string())
    };
    let custom_css = {
        config_arc
            .read()
            .map(|c| c.custom_css_path.clone())
            .unwrap_or(None)
    };
    let theming = Arc::new(ThemeManager::with_config(&active_theme, custom_css));
    let tab_manager = Arc::new(notion::tab_manager::TabManager::<tauri::Wry>::new());

    // Tauri Application Context
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            notion::i18n::get_translation,
            notion::i18n::set_locale,
            notion::spellcheck::check_spelling,
            notion::spellcheck::get_spelling_suggestions,
            notion::commands::update_tab_state,
            notion::commands::get_window_tabs,
            notion::commands::switch_tab,
            notion::commands::close_tab,
            notion::commands::new_tab,
            notion::commands::minimize_window,
            notion::commands::maximize_window,
            notion::commands::close_window,
            notion::commands::check_for_updates_command,
            notion::commands::get_app_config,
            notion::commands::update_app_config,
            notion::commands::open_config_directory,
            notion::commands::toggle_menu_bar_command
        ])
        .setup(move |app| {
            // Initialize modules in Tauri state FIRST as trait objects where expected
            app.manage::<Arc<dyn notion::traits::PolicyEnforcer>>(policy);
            app.manage::<Arc<dyn notion::traits::ThemingEngine<tauri::Wry>>>(theming);
            app.manage::<Arc<dyn notion::traits::TabOrchestrator<tauri::Wry>>>(tab_manager);
            app.manage(config_arc.clone());
            app.manage(app_state);
            app.manage(I18nManager::new());
            app.manage(SpellcheckManager::new());
            app.manage(app_secret_arc.clone());

            let handle = app.handle().clone();

            // Native Menu Setup
            let _ = notion::menu::create_main_menu(&handle);

            // Spawn the main window directly via Tauri WindowController
            match notion::window_controller::WindowController::<tauri::Wry>::new(&handle) {
                Ok(wc) => {
                    wc.setup_listeners(handle.clone());
                    if let Err(e) = wc.setup_tabs(&handle) {
                        log::error!("Failed to set up tabs: {}", e);
                    }
                    log::info!("WindowController initialized and set up.");
                }
                Err(e) => {
                    log::error!("Failed to create WindowController: {}", e);
                }
            }

            if let Err(e) = notion::tray::create(&handle) {
                log::warn!("Failed to initialize system tray icon: {}", e);
            }

            // Background auto-updater task: checks for updates 5s after launch (release builds only)
            #[cfg(not(debug_assertions))]
            {
                let updater_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    if let Err(e) = notion::updater::check_for_updates(updater_handle).await {
                        log::warn!("Auto-updater background check failed: {}", e);
                    }
                });
            }

            log::info!("Tauri background layer initialized.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
