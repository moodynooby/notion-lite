use crate::config::NotionConfig;
use crate::policy::{is_notion_host, is_trusted_oauth_host};
use crate::state::AppState;
use std::sync::Arc;
use tauri::Manager;

// Helper function to refresh tab UI only on the active visible webview
fn refresh_active_tab_ui(
    app: &tauri::AppHandle<tauri::Wry>,
    window_id: &str,
    active_tab_id: Option<&str>,
) {
    if let Some(w) = app.get_window(window_id) {
        if let Some(tab_id) = active_tab_id {
            if let Some(wv) = w.get_webview(tab_id) {
                let _ = wv.eval("if (window.NotionLite && window.NotionLite.refreshTabs) window.NotionLite.refreshTabs();");
            }
        }
    }
}

// Helper function to check if the command invocation origin is trusted
fn is_trusted_origin<R: tauri::Runtime>(webview: &tauri::Webview<R>) -> bool {
    if let Ok(url) = webview.url() {
        if (url.scheme() == "tauri" || url.scheme() == "wry") && url.host_str() == Some("localhost")
        {
            return true;
        }
        if url.scheme() == "https" {
            if let Some(host) = url.host_str() {
                if is_notion_host(host) || is_trusted_oauth_host(host) {
                    return true;
                }
            }
        }
        let origin = format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
        log::warn!("Untrusted origin: {} attempted to invoke command.", origin);
        false
    } else {
        log::warn!("Could not determine origin for command invocation (webview URL not found).");
        false
    }
}

#[tauri::command]
pub fn get_window_tabs(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    state: tauri::State<'_, Arc<std::sync::RwLock<AppState>>>,
) -> Vec<crate::state::TabState> {
    if !is_trusted_origin(&webview) {
        return Vec::new(); // Deny access for untrusted origins
    }
    log::debug!("get_window_tabs called from origin: {:?}", webview.url());
    let app_state = state.read().expect("AppState read lock poisoned");
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
pub fn switch_tab(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    orchestrator: tauri::State<'_, Arc<dyn crate::traits::TabOrchestrator<tauri::Wry>>>,
    state: tauri::State<'_, Arc<std::sync::RwLock<AppState>>>,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("switch_tab called for tab: {}", tab_id);
    let _ = orchestrator.show_tab(&tab_id);

    {
        let mut app_state = state.write().expect("AppState write lock poisoned");
        for window_state in app_state.windows.values_mut() {
            if window_state.tab_ids.contains(&tab_id) {
                window_state.active_tab_id = Some(tab_id.clone());
                break;
            }
        }
        for (id, t) in app_state.tabs.iter_mut() {
            t.is_active = id == &tab_id;
        }
        crate::state::persist_state(&app);
    }

    refresh_active_tab_ui(&app, "main", Some(&tab_id));
}

#[tauri::command]
pub fn close_tab(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    orchestrator: tauri::State<'_, Arc<dyn crate::traits::TabOrchestrator<tauri::Wry>>>,
    state: tauri::State<'_, Arc<std::sync::RwLock<AppState>>>,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("close_tab called for tab: {}", tab_id);
    let _ = orchestrator.destroy_tab(&tab_id);
    let mut new_active_id = None;

    {
        let mut app_state = state.write().expect("AppState write lock poisoned");
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
                new_active_id = window_state.active_tab_id.clone();
                let active_id_opt = window_state.active_tab_id.clone();
                for (id, t) in app_state.tabs.iter_mut() {
                    t.is_active = Some(id.as_str()) == active_id_opt.as_deref();
                }
                break;
            }
        }

        crate::state::persist_state(&app);
    }

    refresh_active_tab_ui(&app, "main", new_active_id.as_deref());
}

#[tauri::command]
pub fn new_tab(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    orchestrator: tauri::State<'_, Arc<dyn crate::traits::TabOrchestrator<tauri::Wry>>>,
    config: tauri::State<'_, Arc<std::sync::RwLock<NotionConfig>>>,
    state: tauri::State<'_, Arc<std::sync::RwLock<AppState>>>,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    log::info!("new_tab called for window: {}", window_id);
    let notion_url = config
        .read()
        .map(|c| c.get_start_url().to_string())
        .unwrap_or_else(|_| "https://app.notion.com".to_string());
    if let Ok(new_id) = orchestrator.create_tab(&app, &window_id, &notion_url) {
        {
            let mut app_state = state.write().expect("AppState write lock poisoned");
            if let Some(w_state) = app_state.windows.get_mut(&window_id) {
                w_state.tab_ids.push(new_id.clone());
                w_state.active_tab_id = Some(new_id.clone());
            }
            for t in app_state.tabs.values_mut() {
                t.is_active = false;
            }
            app_state.tabs.insert(
                new_id.clone(),
                crate::state::TabState {
                    id: new_id.clone(),
                    title: "Notion".to_string(),
                    url: notion_url.to_string(),
                    is_active: true,
                    is_pinned: false,
                },
            );
            crate::state::persist_state(&app);
        }

        let _ = orchestrator.show_tab(&new_id);
        refresh_active_tab_ui(&app, &window_id, Some(&new_id));
    }
}

#[tauri::command]
pub fn update_tab_state(
    webview: tauri::Webview<tauri::Wry>,
    tab_id: String,
    title: String,
    url: String,
    state: tauri::State<'_, Arc<std::sync::RwLock<AppState>>>,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }

    let mut app_state = state.write().expect("AppState write lock poisoned");

    let safe_url = if url.contains("unsupported-browser.html") {
        "https://app.notion.com".to_string()
    } else {
        url.clone()
    };

    // Check if tab already exists, and if title and url haven't changed, return early
    if let Some(existing_tab) = app_state.tabs.get(&tab_id) {
        if existing_tab.title == title && existing_tab.url == safe_url {
            return;
        }
    }

    app_state.tabs.insert(
        tab_id.clone(),
        crate::state::TabState {
            id: tab_id.clone(),
            title: title.clone(),
            url: safe_url,
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

    crate::state::persist_state(&app);
    log::debug!(
        "[notion-state] Updated tab {} (title: {}, url: {})",
        tab_id,
        title,
        url
    );
}

#[tauri::command]
pub fn minimize_window(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    if let Some(window) = app.get_window(&window_id) {
        let _ = window.minimize();
    }
}

#[tauri::command]
pub fn maximize_window(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    app: tauri::AppHandle<tauri::Wry>,
) {
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
pub fn close_window(
    webview: tauri::Webview<tauri::Wry>,
    window_id: String,
    app: tauri::AppHandle<tauri::Wry>,
) {
    if !is_trusted_origin(&webview) {
        return; // Deny access for untrusted origins
    }
    if let Some(window) = app.get_window(&window_id) {
        let _ = window.close();
    }
}

#[tauri::command]
pub async fn check_for_updates_command(app: tauri::AppHandle<tauri::Wry>) -> Result<bool, String> {
    log::info!("Manual update check command invoked");
    crate::updater::check_for_updates(app)
        .await
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConfigDto {
    pub active_theme: String,
    pub show_menu_bar: bool,
    pub restore_tabs: bool,
    pub start_url: String,
    pub version: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateConfigPayload {
    pub active_theme: Option<String>,
    pub show_menu_bar: Option<bool>,
    pub restore_tabs: Option<bool>,
    pub start_url: Option<String>,
}

#[tauri::command]
pub fn get_app_config(
    webview: tauri::Webview<tauri::Wry>,
    config_state: tauri::State<'_, Arc<std::sync::RwLock<NotionConfig>>>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<ConfigDto, String> {
    if !is_trusted_origin(&webview) {
        return Err("Untrusted origin".to_string());
    }
    let config = config_state.read().map_err(|e| e.to_string())?;
    Ok(ConfigDto {
        active_theme: crate::theming::normalize_theme_name(&config.active_theme).to_string(),
        show_menu_bar: config.show_menu_bar,
        restore_tabs: config.restore_tabs,
        start_url: config.get_start_url().to_string(),
        version: app.package_info().version.to_string(),
    })
}

#[tauri::command]
pub fn update_app_config(
    webview: tauri::Webview<tauri::Wry>,
    payload: UpdateConfigPayload,
    config_state: tauri::State<'_, Arc<std::sync::RwLock<NotionConfig>>>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    if !is_trusted_origin(&webview) {
        return Err("Untrusted origin".to_string());
    }

    let mut config = config_state.write().map_err(|e| e.to_string())?;
    let mut theme_to_apply = None;
    let mut menu_visibility = None;

    if let Some(new_theme) = payload.active_theme {
        let new_theme = crate::theming::normalize_theme_name(&new_theme).to_string();
        if config.active_theme != new_theme {
            config.active_theme = new_theme.clone();
            theme_to_apply = Some(new_theme);
        }
    }

    if let Some(show_menu) = payload.show_menu_bar {
        config.show_menu_bar = show_menu;
        menu_visibility = Some(show_menu);
    }

    if let Some(restore) = payload.restore_tabs {
        config.restore_tabs = restore;
    }

    if let Some(url) = payload.start_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            config.start_url = Some(trimmed.to_string());
        }
    }

    config.save().map_err(|e| e.to_string())?;
    drop(config);

    if let Some(theme) = theme_to_apply {
        crate::menu::apply_theme(&app, &theme);
    }
    if let Some(visible) = menu_visibility {
        if let Err(e) = crate::menu::set_menu_bar_visible(&app, visible) {
            log::warn!("Failed to set menu bar visibility: {}", e);
        }
    }

    log::info!("Updated and saved app configuration successfully");
    Ok(())
}

#[tauri::command]
pub fn open_config_directory(
    webview: tauri::Webview<tauri::Wry>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<(), String> {
    if !is_trusted_origin(&webview) {
        return Err("Untrusted origin".to_string());
    }
    use tauri_plugin_shell::ShellExt;
    let config_dir = NotionConfig::config_dir().to_string_lossy().to_string();
    #[allow(deprecated)]
    app.shell()
        .open(config_dir, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn toggle_menu_bar_command(
    webview: tauri::Webview<tauri::Wry>,
    app: tauri::AppHandle<tauri::Wry>,
) -> Result<bool, String> {
    if !is_trusted_origin(&webview) {
        return Err("Untrusted origin".to_string());
    }
    Ok(crate::menu::toggle_menu_bar(&app))
}
