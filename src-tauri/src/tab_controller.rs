use crate::traits::{PolicyEnforcer, ThemingEngine};
use std::sync::Arc;
use tauri::webview::Webview;
use tauri::{AppHandle, Manager, Runtime, Url, WebviewBuilder, WebviewUrl};

pub struct TabController<R: Runtime> {
    pub tab_id: String,
    pub window_id: String,
    pub webview: Webview<R>,
}

impl<R: Runtime> TabController<R> {
    pub fn new(
        app: &AppHandle<R>,
        window_id: &str,
        tab_id: String,
        url_str: &str,
    ) -> tauri::Result<Self> {
        let policy = app.state::<Arc<dyn PolicyEnforcer>>().inner().clone();

        // Zero-Trust Enforcement: Validate URL before creation
        if !policy.validate_url(url_str) {
            return Err(tauri::Error::AssetNotFound(format!(
                "Zero-Trust Policy Blocked: {}",
                url_str
            )));
        }

        let window = app
            .get_window(window_id)
            .ok_or(tauri::Error::AssetNotFound(format!(
                "Window {} not found",
                window_id
            )))?;

        let url = url_str
            .parse::<Url>()
            .map_err(|e| tauri::Error::AssetNotFound(e.to_string()))?;

        // Create a new webview for this tab using the secure factory
        let webview_builder =
            create_secure_webview_builder(app, &tab_id, &url, window_id, policy.clone());

        let inner_size = window.inner_size()?;
        let size = if inner_size.width > 0 && inner_size.height > 0 {
            tauri::Size::Physical(inner_size)
        } else {
            tauri::Size::Logical(tauri::LogicalSize::new(1200.0, 768.0))
        };
        let webview =
            window.add_child(webview_builder, tauri::LogicalPosition::new(0.0, 0.0), size)?;
        let _ = webview.set_auto_resize(true);
        // Start hidden by default; TabOrchestrator manages visibility
        let _ = webview.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            -10000.0, -10000.0,
        )));
        let _ = webview.set_size(tauri::Size::Logical(tauri::LogicalSize::new(0.0, 0.0)));
        let _ = webview.hide();

        log::info!("Created tab webview: {} in window: {}", tab_id, window_id);

        // Inject theme from config (not hardcoded)
        let theming = app.state::<Arc<dyn ThemingEngine<R>>>();
        let active_theme = theming.get_active_theme();
        theming.inject_theme(&webview, &active_theme);

        Ok(Self {
            tab_id,
            window_id: window_id.to_string(),
            webview,
        })
    }

    pub fn show(&self) -> tauri::Result<()> {
        let _ = self
            .webview
            .set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
                0.0, 0.0,
            )));
        let w = self.webview.window();
        if let Ok(size) = w.inner_size() {
            let _ = self.webview.set_size(tauri::Size::Physical(size));
        }
        self.webview.show()?;
        let _ = self.webview.set_focus();
        let _ = self.webview.eval("if (window.NotionLite && window.NotionLite.refreshTabs) window.NotionLite.refreshTabs();");
        Ok(())
    }

    pub fn hide(&self) -> tauri::Result<()> {
        let _ = self
            .webview
            .set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
                -10000.0, -10000.0,
            )));
        let _ = self
            .webview
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(0.0, 0.0)));
        self.webview.hide()?;
        Ok(())
    }

    pub fn destroy(&self) -> tauri::Result<()> {
        log::info!("Destroying tab: {}", self.tab_id);
        self.webview.close()?;
        Ok(())
    }
}

/// Routes secure popup requests into the application's internal TabManager.
/// Guaranteeing that any nested popups (e.g. nested OAuth flows) inherit
/// the exact same zero-trust `on_navigation` and `on_new_window` policies
/// as their parent window via the TabController factory.
pub fn spawn_secure_popup<R: Runtime>(app: &AppHandle<R>, url: Url) {
    log::info!(
        "Intercepted popup request. Routing into a secure in-app tab: {}",
        url.as_str()
    );

    // Instead of spawning a completely disconnected OS window, dispatch the popup
    // into our managed TabOrchestrator. This keeps the application bounded strictly
    // to a single window and enforces all Zero-Trust policies recursively since
    // create_tab() uses the TabController factory.
    if let Some(orchestrator) = app.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>() {
        if let Err(e) = orchestrator.inner().create_tab(app, "main", url.as_str()) {
            log::error!("Zero-Trust: Failed to route popup into managed tab: {}", e);
        }
    } else {
        log::error!("Zero-Trust: Cannot spawn tab securely. TabOrchestrator missing from state.");
    }
}

/// A centralized factory for creating WebviewBuilders with guaranteed security listeners.
/// Ensures all created webviews (tabs or popups) enforce on_navigation and on_new_window policies.
pub fn create_secure_webview_builder<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    url: &Url,
    window_id: &str,
    policy: Arc<dyn PolicyEnforcer>,
) -> WebviewBuilder<R> {
    let (theme_name, custom_css) =
        if let Some(theming) = app.try_state::<Arc<dyn ThemingEngine<R>>>() {
            (theming.get_active_theme(), theming.get_custom_css())
        } else {
            ("dracula".to_string(), String::new())
        };

    // Theme CSS is part of the initialization script so it is re-applied on
    // every document load. A one-shot `eval()` after webview creation lands in
    // the pre-navigation document and is lost as soon as Notion commits.
    let mut script = get_injected_tab_bar_js(label, window_id, &theme_name);
    script.push('\n');
    script.push_str(&crate::theming::theme_init_script(&theme_name));
    if !custom_css.is_empty() {
        script.push('\n');
        script.push_str(&custom_css);
    }

    let webview_builder = WebviewBuilder::new(label, WebviewUrl::External(url.clone()))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
        .initialization_script(&script);

    let nav_app = app.clone();
    let nav_policy = policy.clone();
    let popup_app = app.clone();
    let popup_policy = policy.clone();
    let window_id_owned = window_id.to_string();

    webview_builder
        .on_navigation(move |url| {
            let window_id = &window_id_owned;
            let url_str = url.as_str();

            // Intercept custom window control actions
            let action_opt = if url_str.starts_with("notion-lite-action://") {
                Some(url_str.strip_prefix("notion-lite-action://").unwrap_or(""))
            } else if url_str.starts_with("notion-action://") {
                Some(url_str.strip_prefix("notion-action://").unwrap_or(""))
            } else if url_str.starts_with("lotion-action://") {
                Some(url_str.strip_prefix("lotion-action://").unwrap_or(""))
            } else {
                None
            };

            if let Some(action) = action_opt {
                log::info!("Intercepted action: {}", action);

                if let Some(w) = nav_app.get_window(window_id) {
                    if action == "window:close" {
                        let _ = w.close();
                    } else if action == "window:minimize" {
                        let _ = w.minimize();
                    } else if action == "window:maximize" {
                        if let Ok(true) = w.is_maximized() {
                            let _ = w.unmaximize();
                        } else {
                            let _ = w.maximize();
                        }
                    } else if action == "tab:new" {
                        let notion_url = nav_app
                            .try_state::<Arc<std::sync::RwLock<crate::config::NotionConfig>>>()
                            .and_then(|c| {
                                c.read().ok().map(|cfg| cfg.get_start_url().to_string())
                            })
                            .unwrap_or_else(|| "https://app.notion.com".to_string());
                        if let Some(orchestrator) =
                            nav_app.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>()
                        {
                            if let Ok(new_id) =
                                orchestrator.inner().create_tab(&nav_app, window_id, &notion_url)
                            {
                                if let Some(state_lock) = nav_app.try_state::<Arc<
                                    std::sync::RwLock<crate::state::AppState>,
                                >>() {
                                    let mut app_state = state_lock.write().expect("AppState write lock poisoned");
                                    if let Some(w_state) = app_state.windows.get_mut(window_id) {
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
                                    crate::state::persist_state(&nav_app, &app_state);
                                }

                                let _ = orchestrator.inner().show_tab(&new_id);

                                if let Some(w) = nav_app.get_window(window_id) {
                                    if let Some(wv) = w.get_webview(&new_id) {
                                        let _ = wv.eval("if (window.NotionLite && window.NotionLite.refreshTabs) window.NotionLite.refreshTabs();");
                                    }
                                }
                            }
                        }
                    } else if let Some(tab_id_to_close) = action.strip_prefix("tab:close:") {
                        if let Some(orchestrator) =
                            nav_app.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>()
                        {
                            let _ = orchestrator.inner().destroy_tab(tab_id_to_close);
                            let mut new_active_id = None;
                            if let Some(state_lock) = nav_app.try_state::<Arc<
                                std::sync::RwLock<crate::state::AppState>,
                            >>() {
                                let mut app_state = state_lock.write().expect("AppState write lock poisoned");
                                app_state.tabs.remove(tab_id_to_close);
                                for window_state in app_state.windows.values_mut() {
                                    if let Some(pos) = window_state.tab_ids.iter().position(|id| id == tab_id_to_close) {
                                        window_state.tab_ids.remove(pos);
                                        if window_state.active_tab_id.as_deref() == Some(tab_id_to_close) {
                                            window_state.active_tab_id = window_state.tab_ids.last().cloned();
                                            if let Some(new_active_tab_id) = &window_state.active_tab_id {
                                                let _ = orchestrator.inner().show_tab(new_active_tab_id);
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
                                crate::state::persist_state(&nav_app, &app_state);
                            }
                            if let Some(w) = nav_app.get_window(window_id) {
                                if let Some(ref act_id) = new_active_id {
                                    if let Some(wv) = w.get_webview(act_id) {
                                        let _ = wv.eval("if (window.NotionLite && window.NotionLite.refreshTabs) window.NotionLite.refreshTabs();");
                                    }
                                }
                            }
                        }
                    } else if let Some(tab_id_to_switch) = action.strip_prefix("tab:switch:") {
                        if let Some(orchestrator) =
                            nav_app.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>()
                        {
                            let _ = orchestrator.inner().show_tab(tab_id_to_switch);
                            if let Some(state_lock) = nav_app.try_state::<Arc<
                                std::sync::RwLock<crate::state::AppState>,
                            >>() {
                                let mut app_state = state_lock.write().expect("AppState write lock poisoned");
                                for window_state in app_state.windows.values_mut() {
                                    if window_state.tab_ids.iter().any(|id| id == tab_id_to_switch) {
                                        window_state.active_tab_id = Some(tab_id_to_switch.to_string());
                                        break;
                                    }
                                }
                                for (id, t) in app_state.tabs.iter_mut() {
                                    t.is_active = id == tab_id_to_switch;
                                }
                                crate::state::persist_state(&nav_app, &app_state);
                            }
                            if let Some(w) = nav_app.get_window(window_id) {
                                if let Some(wv) = w.get_webview(tab_id_to_switch) {
                                    let _ = wv.eval("if (window.NotionLite && window.NotionLite.refreshTabs) window.NotionLite.refreshTabs();");
                                }
                            }
                        }
                    } else if action == "config:open_dir" {
                        let config_dir = crate::config::NotionConfig::config_dir();
                        use tauri_plugin_opener::OpenerExt;
                        let _ = nav_app.opener().open_path(config_dir.to_string_lossy().to_string(), None::<&str>);
                    } else {
                        log::warn!("Unknown Lotion action: {}", action);
                    }
                }
                return false; // Prevent actual navigation
            }

            if nav_policy.validate_url(url_str) {
                true // Allow internal navigation to Notion
            } else if nav_policy.validate_external_link(url_str) {
                log::info!(
                    "Zero-Trust: Opening validated external link in default browser: {}",
                    url_str
                );
                use tauri_plugin_opener::OpenerExt;
                let _ = nav_app.opener().open_url(url_str, None::<&str>);
                false // Block navigation in the webview
            } else {
                log::warn!(
                    "Zero-Trust: BLOCKED unauthorized navigation attempt to: {}",
                    url_str
                );
                false // Block everything else
            }
        })
        .on_new_window(move |url, _| {
            use tauri_plugin_opener::OpenerExt;
            let url_str = url.as_str();

            if popup_policy.should_route_popup_to_system_browser(url_str) {
                if popup_policy.validate_external_link(url_str) {
                    log::info!(
                        "Routing new window request (popup) to system browser: {}",
                        url_str
                    );
                    let _ = popup_app.opener().open_url(url_str, None::<&str>);
                } else {
                    log::warn!(
                        "Zero-Trust: BLOCKED unauthorized popup attempt to: {}",
                        url_str
                    );
                }
                tauri::webview::NewWindowResponse::Deny
            } else {
                // Spawn a controlled popup using the recursive secure factory
                spawn_secure_popup(&popup_app, url.clone());
                tauri::webview::NewWindowResponse::Deny
            }
        })
}

/// Constructs the injected JavaScript responsible for the Notion-styled
/// multi-tab bar, navigation controls, preferences gear icon, and the
/// interactive in-app Preferences modal overlay.
const TAB_UI_JS: &str = include_str!("tab_ui.js");

pub fn get_injected_tab_bar_js(tab_id: &str, window_id: &str, initial_theme: &str) -> String {
    TAB_UI_JS
        .replace("__TAB_ID__", tab_id)
        .replace("__WINDOW_ID__", window_id)
        .replace("__THEME__", initial_theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_injected_tab_bar_js_formatting() {
        let js = get_injected_tab_bar_js("tab-123", "main", "dracula");
        assert!(js.contains("tab-123"));
        assert!(js.contains("main"));
        assert!(js.contains("dracula"));
        assert!(js.contains("notion-lite-header"));
        assert!(js.contains("notion-lite-prefs-modal"));
    }
}
