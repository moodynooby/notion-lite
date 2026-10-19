use crate::litebox::LiteBox;
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
        _litebox: Arc<LiteBox>,
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
        let _ = webview.show();

        log::info!("Created tab webview: {} in window: {}", tab_id, window_id);

        // Inject theme from config (not hardcoded)
        let theming = app.state::<Arc<dyn ThemingEngine<R>>>();
        let active_theme = theming.get_active_theme();
        theming.inject_theme(&webview, &active_theme);

        // Inject title observer and custom Mac-style Window Controls
        let title_observer_js = format!(
            r#"
            (function() {{
                const tabId = '{0}';

                // 1. Title Observer
                let lastTitle = document.title;
                const observer = new MutationObserver(function() {{
                    if (document.title !== lastTitle) {{
                        lastTitle = document.title;
                        if (window.__TAURI__) {{
                            window.__TAURI__.invoke('update_tab_state', {{
                                tabId: tabId,
                                title: lastTitle,
                                url: window.location.href
                            }});
                        }}
                    }}
                }});
                observer.observe(document.querySelector('title') || document.head, {{
                    subtree: true, characterData: true, childList: true
                }});

            }})();
        "#,
            tab_id
        );
        let _ = webview.eval(&title_observer_js);

        Ok(Self {
            tab_id,
            window_id: window_id.to_string(),
            webview,
        })
    }

    pub fn load_url(&self, app: &AppHandle, url: &str) -> tauri::Result<()> {
        let policy = app.state::<Arc<dyn PolicyEnforcer>>();

        // Zero-Trust Enforcement: Validate URL before navigation
        if !policy.validate_url(url) {
            log::warn!("Zero-Trust Policy Blocked navigation to: {}", url);
            return Ok(()); // Fail closed (don't navigate)
        }

        log::info!("Tab {}: Loading URL {}", self.tab_id, url);
        let url = url
            .parse::<Url>()
            .map_err(|e| tauri::Error::AssetNotFound(e.to_string()))?;
        self.webview.navigate(url)?;
        Ok(())
    }

    pub fn show(&self) -> tauri::Result<()> {
        self.webview.show()?;
        Ok(())
    }

    pub fn hide(&self) -> tauri::Result<()> {
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
pub fn spawn_secure_popup<R: Runtime>(
    app: &AppHandle<R>,
    _policy: Arc<dyn PolicyEnforcer>,
    url: Url,
) {
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
    let webview_builder = WebviewBuilder::new(label, WebviewUrl::External(url.clone()));

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
                    match action {
                        "window:close" => {
                            let _ = w.close();
                        }
                        "window:minimize" => {
                            let _ = w.minimize();
                        }
                        "window:maximize" => {
                            if let Ok(true) = w.is_maximized() {
                                let _ = w.unmaximize();
                            } else {
                                let _ = w.maximize();
                            }
                        }
                        "tab:new" => {
                            let notion_url = nav_app
                                .try_state::<crate::config::LotionConfig>()
                                .map(|c| c.get_start_url().to_string())
                                .unwrap_or_else(|| "https://app.notion.com".to_string());
                            if let Some(orchestrator) =
                                nav_app.try_state::<Arc<dyn crate::traits::TabOrchestrator<R>>>()
                            {
                                if let Ok(new_id) =
                                    orchestrator.inner().create_tab(&nav_app, window_id, &notion_url)
                                {
                                    let _ = orchestrator.inner().show_tab(&new_id);

                                    // Update AppState
                                    if let Some(state_lock) = nav_app.try_state::<Arc<
                                        tokio::sync::Mutex<crate::state::AppState>,
                                    >>() {
                                        let mut app_state = state_lock.blocking_lock();
                                        if let Some(w_state) = app_state.windows.get_mut(window_id) {
                                            w_state.tab_ids.push(new_id);
                                            // Get app_secret from state
                                            if let Some(app_secret_state) = nav_app.try_state::<Arc<Vec<u8>>>() {
                                                let _ = app_state.save_to_disk(app_secret_state.inner().as_slice());
                                            } else {
                                                log::error!("Zero-Trust: App secret not found in state when trying to save AppState after new tab creation.");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            log::warn!("Unknown Lotion action: {}", action);
                        }
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
                use tauri_plugin_shell::ShellExt;
                #[allow(deprecated)]
                let _ = nav_app.shell().open(url_str.to_string(), None);
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
            use tauri_plugin_shell::ShellExt;
            let url_str = url.as_str();

            if popup_policy.should_route_popup_to_system_browser(url_str) {
                if popup_policy.validate_external_link(url_str) {
                    log::info!(
                        "Routing new window request (popup) to system browser: {}",
                        url_str
                    );
                    #[allow(deprecated)]
                    let _ = popup_app.shell().open(url_str.to_string(), None);
                } else {
                    log::warn!(
                        "Zero-Trust: BLOCKED unauthorized popup attempt to: {}",
                        url_str
                    );
                }
                tauri::webview::NewWindowResponse::Deny
            } else {
                // Spawn a controlled popup using the recursive secure factory
                spawn_secure_popup(&popup_app, popup_policy.clone(), url.clone());
                tauri::webview::NewWindowResponse::Deny
            }
        })
}
