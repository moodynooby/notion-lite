use notion_lite as notion;
use notion::policy::PolicyManager;
use notion::security::SecurityModule;
use notion::theming::ThemeManager;

use notion::config::NotionConfig;
use notion::i18n::I18nManager;
use notion::spellcheck::SpellcheckManager;
use notion::state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;
use std::fs::{self, File};
use std::io::{Read, Write};
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt; // Specific import for unix permissions
use rand::RngCore;

const SECRET_FILE_NAME: &str = "secret_key";

// Helper function to get or create the application secret
fn get_or_create_app_secret() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let config_base = dirs::config_dir().ok_or("Could not find config directory")?;
    let secret_dir = config_base.join("notion-lite");
    let secret_path = secret_dir.join(SECRET_FILE_NAME);

    if secret_path.exists() {
        let mut file = File::open(&secret_path)?;
        let mut secret = vec![0u8; 32];
        file.read_exact(&mut secret)?;
        log::info!("Application secret loaded from {}", secret_path.display());
        Ok(secret)
    } else {
        // Check for legacy notion or lotion-rs secret key to seamlessly preserve existing sessions
        let legacy_notion_path = config_base.join("notion").join(SECRET_FILE_NAME);
        let legacy_lotion_path = config_base.join("lotion-rs").join(SECRET_FILE_NAME);
        let legacy_path = if legacy_notion_path.exists() {
            Some(legacy_notion_path)
        } else if legacy_lotion_path.exists() {
            Some(legacy_lotion_path)
        } else {
            None
        };

        if let Some(legacy_path) = legacy_path {
            log::info!("Migrating legacy application secret from {}", legacy_path.display());
            fs::create_dir_all(&secret_dir)?;
            let mut file = File::open(&legacy_path)?;
            let mut secret = vec![0u8; 32];
            file.read_exact(&mut secret)?;
            
            let mut new_file = File::create(&secret_path)?;
            #[cfg(target_family = "unix")]
            {
                new_file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            new_file.write_all(&secret)?;
            return Ok(secret);
        }

        log::info!("Generating new application secret at {}", secret_path.display());
        fs::create_dir_all(&secret_dir)?;
        
        let mut secret = vec![0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret);

        let mut file = File::create(&secret_path)?;
        #[cfg(target_family = "unix")]
        {
            // Set permissions to 0o600 (read/write only for owner)
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&secret)?;
        Ok(secret)
    }
}

// Helper function to check if the command invocation origin is trusted
fn is_trusted_origin<R: tauri::Runtime>(webview: &tauri::Webview<R>) -> bool {
    let trusted_origins = vec![
        "https://www.notion.so",
        "https://notion.so",
        "https://www.notion.com",
        "https://notion.com",
        "tauri://localhost", // For local development/devtools
    ];

    if let Ok(url) = webview.url() {
        let origin = format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
        let is_trusted = trusted_origins.iter().any(|o| origin.starts_with(o));
        if !is_trusted {
            log::warn!("Untrusted origin: {} attempted to invoke command.", origin);
        }
        is_trusted
    } else {
        log::warn!("Could not determine origin for command invocation (webview URL not found).");
        false
    }
}

#[tauri::command]
fn get_window_tabs(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    state: tauri::State<'_, Arc<tokio::sync::Mutex<AppState>>>,
) -> Vec<notion::state::TabState> {
    if !is_trusted_origin(&webview) {
        return Vec::new(); // Deny access for untrusted origins
    }
    log::info!("get_window_tabs called from origin: {:?}", webview.url());
    let app_state = state.blocking_lock();
    if let Some(w_state) = app_state.windows.get(&window_id) {
        w_state
            .tab_ids
            .iter()
            .filter_map(|id| app_state.tabs.get(id))
            .cloned()
            .collect()
    } else {
        Vec::new()
    }
}

#[tauri::command]
fn switch_tab(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    orchestrator: tauri::State<'_, Arc<dyn notion::traits::TabOrchestrator<tauri::Wry>>>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("switch_tab called for tab: {}", tab_id);
    let _ = orchestrator.show_tab(&tab_id);
}

#[tauri::command]
fn close_tab(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    orchestrator: tauri::State<'_, Arc<dyn notion::traits::TabOrchestrator<tauri::Wry>>>,
    state: tauri::State<'_, Arc<tokio::sync::Mutex<AppState>>>,
    app_secret_state: tauri::State<'_, Arc<Vec<u8>>>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("close_tab called for tab: {}", tab_id);
    let _ = orchestrator.destroy_tab(&tab_id);

    let mut app_state = state.blocking_lock();
    app_state.tabs.remove(&tab_id);

    // Remove tab_id from its window
    for window_state in app_state.windows.values_mut() {
        if let Some(pos) = window_state.tab_ids.iter().position(|id| id == &tab_id) {
            window_state.tab_ids.remove(pos);
            // If the closed tab was active, activate another tab if available
            if window_state.active_tab_id.as_deref() == Some(&tab_id) {
                window_state.active_tab_id = window_state.tab_ids.last().cloned();
                if let Some(new_active_tab_id) = &window_state.active_tab_id {
                    let _ = orchestrator.show_tab(new_active_tab_id);
                }
            }
            break;
        }
    }

    let _ = app_state.save_to_disk(app_secret_state.inner().as_slice());
}

#[tauri::command]
fn new_tab(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    orchestrator: tauri::State<'_, Arc<dyn notion::traits::TabOrchestrator<tauri::Wry>>>,
    state: tauri::State<'_, Arc<tokio::sync::Mutex<AppState>>>,
    app_secret_state: tauri::State<'_, Arc<Vec<u8>>>,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("new_tab called for window: {}", window_id);
    let notion_url = "https://www.notion.so";
    if let Ok(new_id) = orchestrator.create_tab(&app, &window_id, notion_url) {
        let _ = orchestrator.show_tab(&new_id);

        let mut app_state = state.blocking_lock();
        if let Some(w_state) = app_state.windows.get_mut(&window_id) {
            w_state.tab_ids.push(new_id.clone());
            w_state.active_tab_id = Some(new_id.clone());
        }
        app_state.tabs.insert(
            new_id.clone(),
            notion::state::TabState {
                id: new_id,
                title: "Notion Lite".to_string(),
                url: notion_url.to_string(),
                is_active: true,
                is_pinned: false,
            },
        );
        let _ = app_state.save_to_disk(app_secret_state.inner().as_slice());
    }
}

#[tauri::command]
fn update_tab_state(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    title: String,
    url: String,
    state: tauri::State<'_, Arc<tokio::sync::Mutex<AppState>>>,
    app_secret_state: tauri::State<'_, Arc<Vec<u8>>>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }

    let mut app_state = state.blocking_lock();

    // Check if tab already exists, and if title and url haven't changed, return early
    if let Some(existing_tab) = app_state.tabs.get(&tab_id) {
        if existing_tab.title == title && existing_tab.url == url {
            return;
        }
    }

    app_state.tabs.insert(
        tab_id.clone(),
        notion::state::TabState {
            id: tab_id.clone(),
            title: title.clone(),
            url: url.clone(),
            is_active: true,
            is_pinned: false,
        },
    );

    // Find which window this tab belongs to and update active_tab_id
    for window_state in app_state.windows.values_mut() {
        if window_state.tab_ids.contains(&tab_id) {
            window_state.active_tab_id = Some(tab_id.clone());
        }
    }

    let _ = app_state.save_to_disk(app_secret_state.inner().as_slice());
    log::debug!(
        "[notion-state] Updated tab {} (title: {}, url: {})",
        tab_id,
        title,
        url
    );
}

#[tauri::command]
fn minimize_window(webview: tauri::Webview<tauri::Wry>, window_id: String, app: tauri::AppHandle<tauri::Wry>) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    if let Some(window) = app.get_window(&window_id) {
        let _ = window.minimize();
    }
}

#[tauri::command]
fn maximize_window(webview: tauri::Webview<tauri::Wry>, window_id: String, app: tauri::AppHandle<tauri::Wry>) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    if let Some(window) = app.get_window(&window_id) {
        if let Ok(true) = window.is_maximized() {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

#[tauri::command]
fn close_window(webview: tauri::Webview<tauri::Wry>, window_id: String, app: tauri::AppHandle<tauri::Wry>) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    if let Some(window) = app.get_window(&window_id) {
        let _ = window.close();
    }
}

async fn check_for_updates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Auto-updater: Checking for updates from endpoint...");
    let updater = app.updater()?;
    if let Some(update) = updater.check().await? {
        log::info!(
            "Auto-updater: Update available! Version {} -> {}",
            update.current_version,
            update.version
        );
        let mut downloaded: usize = 0;
        update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    log::info!("Auto-updater: Downloaded {} of {:?}", downloaded, content_length);
                },
                || {
                    log::info!("Auto-updater: Download complete, applying update...");
                },
            )
            .await?;
        log::info!("Auto-updater: Update installed successfully. Relaunching application...");
        app.restart();
    } else {
        log::info!("Auto-updater: Application is up to date.");
        Ok(false)
    }
}

#[tauri::command]
async fn check_for_updates_command(app: tauri::AppHandle<tauri::Wry>) -> Result<bool, String> {
    log::info!("Manual update check command invoked");
    check_for_updates(app).await.map_err(|e| e.to_string())
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("NO_AT_BRIDGE", "1");

        // Optional hardware acceleration fallback for problematic GPU drivers
        if std::env::var("NOTION_DISABLE_GPU").map(|v| v == "1").unwrap_or(false) {
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
    let app_secret = get_or_create_app_secret()
        .expect("Failed to get or create application secret");
    let app_secret_arc = Arc::new(app_secret);

    // Load user config
    let config = NotionConfig::load();
    log::info!(
        "Config: theme={}, restore_tabs={}",
        config.active_theme,
        config.restore_tabs
    );

    // Load saved state (if any)
    let app_state = AppState::load_from_disk(&app_secret_arc).unwrap_or_default();
    let app_state = Arc::new(tokio::sync::Mutex::new(app_state));

    // Initialize Concrete Modules
    let security = Arc::new(SecurityModule::new());
    let policy = Arc::new(PolicyManager::new());
    let theming = Arc::new(ThemeManager::with_config(
        &config.active_theme,
        config.custom_css_path.clone(),
    ));
    let tab_manager = Arc::new(notion::tab_manager::TabManager::<tauri::Wry>::new(
        security.litebox.clone(),
    ));

    // Tauri Application Context
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            notion::i18n::get_translation,
            notion::i18n::set_locale,
            notion::spellcheck::check_spelling,
            notion::spellcheck::get_spelling_suggestions,
            update_tab_state,
            get_window_tabs,
            switch_tab,
            close_tab,
            new_tab,
            minimize_window,
            maximize_window,
            close_window,
            check_for_updates_command
        ])
        .setup(move |app| {
            // Initialize modules in Tauri state FIRST as trait objects where expected
            app.manage::<Arc<dyn notion::traits::SecuritySandbox>>(security.litebox.clone());
            app.manage::<Arc<dyn notion::traits::PolicyEnforcer>>(policy);
            app.manage::<Arc<dyn notion::traits::ThemingEngine<tauri::Wry>>>(theming);
            app.manage::<Arc<dyn notion::traits::TabOrchestrator<tauri::Wry>>>(tab_manager);
            app.manage(config);
            app.manage(app_state);
            app.manage(I18nManager::new());
            app.manage(SpellcheckManager::new());
            app.manage(app_secret_arc.clone());

            let handle = app.handle().clone();

            // Native Menu Setup
            let _ = notion::menu::create_main_menu(&handle);

            let security_state = handle
                .state::<Arc<dyn notion::traits::SecuritySandbox>>()
                .inner()
                .clone();

            // Spawn the main window directly via Tauri WindowController
            match notion::window_controller::WindowController::<tauri::Wry>::new(&handle, security_state) {
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

            // Spawn tray icon if supported
            if let Some(icon) = app.default_window_icon() {
                let _ = tauri::tray::TrayIconBuilder::new()
                    .icon(icon.clone())
                    .tooltip("Notion Lite")
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app);
            }

            // Background auto-updater task: checks for updates 5s after launch
            let updater_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Err(e) = check_for_updates(updater_handle).await {
                    log::warn!("Auto-updater background check failed: {}", e);
                }
            });

            log::info!("Tauri background layer initialized.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
