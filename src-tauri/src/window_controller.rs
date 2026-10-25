use crate::traits::TabOrchestrator;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime, Window, WindowBuilder};

pub struct WindowController<R: Runtime> {
    pub window: Window<R>,
}

impl<R: Runtime> WindowController<R> {
    pub fn new(app: &AppHandle<R>) -> tauri::Result<Self> {
        let (initial_width, initial_height, initial_x, initial_y, initial_maximized) = {
            let app_state_lock = app.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
            let app_state = app_state_lock.read().expect("AppState read lock poisoned");
            if let Some(w) = app_state.windows.get("main") {
                let w_val = if w.bounds.width >= 300.0 {
                    w.bounds.width
                } else {
                    1200.0
                };
                let h_val = if w.bounds.height >= 200.0 {
                    w.bounds.height
                } else {
                    768.0
                };
                (w_val, h_val, w.bounds.x, w.bounds.y, w.is_maximized)
            } else if let Some(config_lock) =
                app.try_state::<Arc<std::sync::RwLock<crate::config::NotionConfig>>>()
            {
                let cfg = config_lock.read().ok();
                let w_cfg = cfg.as_ref().map(|c| &c.window);
                let w_val = w_cfg
                    .map(|w| w.width)
                    .filter(|w| *w >= 300.0)
                    .unwrap_or(1200.0);
                let h_val = w_cfg
                    .map(|w| w.height)
                    .filter(|h| *h >= 200.0)
                    .unwrap_or(768.0);
                let x_val = w_cfg.and_then(|w| w.x);
                let y_val = w_cfg.and_then(|w| w.y);
                let max_val = w_cfg.map(|w| w.maximized).unwrap_or(false);
                (w_val, h_val, x_val, y_val, max_val)
            } else {
                (1200.0, 768.0, None, None, false)
            }
        };

        let mut builder = WindowBuilder::new(app, "main")
            .title("Notion Lite")
            .inner_size(initial_width, initial_height)
            .decorations(false)
            .resizable(true);

        if let (Some(x), Some(y)) = (initial_x, initial_y) {
            builder = builder.position(x, y);
        }

        if initial_maximized {
            builder = builder.maximized(true);
        }

        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone())?;
        }

        let window = builder.build()?;

        let show_menu = if let Some(config_lock) =
            app.try_state::<Arc<std::sync::RwLock<crate::config::NotionConfig>>>()
        {
            config_lock.read().map(|c| c.show_menu_bar).unwrap_or(false)
        } else {
            false
        };

        if !show_menu {
            let _ = window.remove_menu();
            let _ = window.hide_menu();
        }

        let _ = window.show();
        let _ = window.set_focus();

        // Ensure window state exists in AppState
        let app_state_lock = app.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
        let mut app_state = app_state_lock
            .write()
            .expect("AppState write lock poisoned");
        if !app_state.windows.contains_key("main") {
            app_state.windows.insert(
                "main".to_string(),
                crate::state::WindowState {
                    id: "main".to_string(),
                    bounds: crate::state::Bounds {
                        x: None,
                        y: None,
                        width: 1200.0,
                        height: 768.0,
                    },
                    is_focused: true,
                    is_maximized: false,
                    is_minimized: false,
                    is_full_screen: false,
                    tab_ids: Vec::new(),
                    active_tab_id: None,
                },
            );
            crate::state::persist_state(app);
        }

        Ok(Self { window })
    }

    pub fn setup_listeners(&self, app_handle: AppHandle<R>) {
        let window_label = self.window.label().to_string();
        let window_clone = self.window.clone();

        self.window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                log::info!(
                    "Window {} close requested, hiding to system tray",
                    window_label
                );
                api.prevent_close();
                let app_state_lock =
                    app_handle.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
                let app_state = app_state_lock.read().expect("AppState read lock poisoned");
                crate::state::persist_state_now(&app_handle, &app_state);
                let _ = window_clone.hide();
                on_window_backgrounded(&app_handle, &window_label);
            }
            tauri::WindowEvent::Focused(focused) => {
                log::debug!("Window {} focused: {}", window_label, focused);
                let app_state_lock =
                    app_handle.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
                let mut app_state = app_state_lock
                    .write()
                    .expect("AppState write lock poisoned");
                if *focused {
                    app_state.focused_window_id = Some(window_label.clone());
                    if let Some(w_state) = app_state.windows.get_mut(&window_label) {
                        w_state.is_focused = true;
                        w_state.is_minimized = false;
                    }
                    drop(app_state);
                    on_window_foregrounded(&app_handle, &window_label);
                } else {
                    let is_minimized = window_clone.is_minimized().unwrap_or(false);
                    let is_visible = window_clone.is_visible().unwrap_or(true);
                    if let Some(w_state) = app_state.windows.get_mut(&window_label) {
                        w_state.is_focused = false;
                        w_state.is_minimized = is_minimized || !is_visible;
                    }
                    drop(app_state);
                    if is_minimized || !is_visible {
                        on_window_backgrounded(&app_handle, &window_label);
                    }
                }
            }
            tauri::WindowEvent::Resized(size) => {
                log::debug!("Window {} resized to {:?}", window_label, size);
                let is_minimized = window_clone.is_minimized().unwrap_or(false);
                if is_minimized {
                    on_window_backgrounded(&app_handle, &window_label);
                }
                if let Some(app_state_lock) =
                    app_handle.try_state::<Arc<std::sync::RwLock<crate::state::AppState>>>()
                {
                    if let Ok(mut app_state) = app_state_lock.inner().write() {
                        if let Some(w_state) = app_state.windows.get_mut(&window_label) {
                            w_state.bounds.width = size.width as f64;
                            w_state.bounds.height = size.height as f64;
                            w_state.is_minimized = is_minimized;
                        }
                    }
                }
            }
            tauri::WindowEvent::Moved(position) => {
                log::debug!("Window {} moved to {:?}", window_label, position);
                if let Some(app_state_lock) =
                    app_handle.try_state::<Arc<std::sync::RwLock<crate::state::AppState>>>()
                {
                    if let Ok(mut app_state) = app_state_lock.inner().write() {
                        if let Some(w_state) = app_state.windows.get_mut(&window_label) {
                            w_state.bounds.x = Some(position.x as f64);
                            w_state.bounds.y = Some(position.y as f64);
                        }
                    }
                }
            }
            _ => {}
        });
    }

    pub fn setup_tabs(&self, app: &AppHandle<R>) -> tauri::Result<()> {
        let tab_manager = app
            .try_state::<Arc<dyn TabOrchestrator<R>>>()
            .ok_or_else(|| {
                tauri::Error::AssetNotFound("TabOrchestrator state not available".into())
            })?
            .inner()
            .clone();

        let (restore_tabs, start_url) = {
            let config = app.state::<Arc<std::sync::RwLock<crate::config::NotionConfig>>>();
            let config = config
                .read()
                .map_err(|e| tauri::Error::AssetNotFound(e.to_string()))?;
            (config.restore_tabs, config.get_start_url().to_string())
        };

        let restored = if restore_tabs {
            self.restore_tabs(app, &tab_manager, &start_url)?
        } else {
            self.clear_saved_tabs(app);
            0
        };

        if restored == 0 {
            self.create_initial_tab(app, &tab_manager, &start_url)?;
        }

        let _ = self.window.show();
        let _ = self.window.set_focus();
        Ok(())
    }

    /// Restores persisted tabs, remapping their old IDs to freshly created ones.
    /// Returns the number of restored tabs.
    fn restore_tabs(
        &self,
        app: &AppHandle<R>,
        tab_manager: &Arc<dyn TabOrchestrator<R>>,
        start_url: &str,
    ) -> tauri::Result<usize> {
        let window_label = self.window.label().to_string();
        let state = app.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();

        let (old_active, old_ids) = {
            let mut app_state = state.write().expect("AppState write lock poisoned");
            let Some(window_state) = app_state.windows.get_mut(&window_label) else {
                return Ok(0);
            };
            let old_active = window_state.active_tab_id.take();
            let old_ids = std::mem::take(&mut window_state.tab_ids);
            (old_active, old_ids)
        };

        let old_tabs: Vec<(String, crate::state::TabState)> = {
            let mut app_state = state.write().expect("AppState write lock poisoned");
            old_ids
                .into_iter()
                .filter_map(|id| app_state.tabs.remove(&id).map(|tab| (id, tab)))
                .collect()
        };

        if old_tabs.is_empty() {
            return Ok(0);
        }

        log::info!(
            "WindowController: Restoring {} tabs from saved state (lazy loading inactive tabs).",
            old_tabs.len()
        );

        let target_active_old_id = old_active
            .filter(|id| old_tabs.iter().any(|(old_id, _)| old_id == id))
            .or_else(|| old_tabs.last().map(|(id, _)| id.clone()));

        let mut remap = std::collections::HashMap::with_capacity(old_tabs.len());
        let mut restored = Vec::with_capacity(old_tabs.len());
        for (old_id, tab) in old_tabs {
            let url = restored_url(&tab.url, start_url);
            let title = restored_title(&tab.title);
            let is_active_tab = target_active_old_id.as_deref() == Some(&old_id);

            let new_id = if is_active_tab {
                // Eagerly instantiate Webview only for the active tab to save CPU & RAM
                tab_manager.create_tab(app, &window_label, &url)?
            } else {
                // Inactive tabs are hibernated: zero Webview creation overhead at launch
                let id = uuid::Uuid::new_v4().to_string();
                tab_manager.register_hibernated_tab(&window_label, id.clone(), &url);
                id
            };

            remap.insert(old_id, new_id.clone());
            restored.push((new_id, title, url, tab.is_pinned));
        }

        let active = {
            let mut app_state = state.write().expect("AppState write lock poisoned");
            let tab_ids: Vec<String> = restored.iter().map(|(id, ..)| id.clone()).collect();
            let active = {
                let Some(window_state) = app_state.windows.get_mut(&window_label) else {
                    return Ok(0);
                };
                window_state.tab_ids = tab_ids;
                let active = target_active_old_id
                    .as_ref()
                    .and_then(|id| remap.get(id).cloned())
                    .or_else(|| window_state.tab_ids.last().cloned());
                window_state.active_tab_id = active.clone();
                active
            };
            for (id, title, url, is_pinned) in &restored {
                app_state.tabs.insert(
                    id.clone(),
                    crate::state::TabState {
                        id: id.clone(),
                        title: title.clone(),
                        url: url.clone(),
                        is_active: active.as_ref() == Some(id),
                        is_pinned: *is_pinned,
                    },
                );
            }
            crate::state::persist_state(app);
            active
        };

        if let Some(active_id) = active {
            let _ = tab_manager.show_tab(&active_id);
        }

        Ok(restored.len())
    }

    /// Drops saved tab state for this window when the user disabled restore.
    fn clear_saved_tabs(&self, app: &AppHandle<R>) {
        let window_label = self.window.label();
        let state = app.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
        let mut app_state = state.write().expect("AppState write lock poisoned");
        if let Some(window_state) = app_state.windows.get_mut(window_label) {
            let old_ids = std::mem::take(&mut window_state.tab_ids);
            window_state.active_tab_id = None;
            for id in old_ids {
                app_state.tabs.remove(&id);
            }
        }
        crate::state::persist_state(app);
    }

    fn create_initial_tab(
        &self,
        app: &AppHandle<R>,
        tab_manager: &Arc<dyn TabOrchestrator<R>>,
        start_url: &str,
    ) -> tauri::Result<()> {
        log::info!(
            "WindowController: Creating initial tab for Notion: {}",
            start_url
        );
        let tab_id = tab_manager.create_tab(app, self.window.label(), start_url)?;

        let state = app.state::<Arc<std::sync::RwLock<crate::state::AppState>>>();
        let mut app_state = state.write().expect("AppState write lock poisoned");
        if let Some(window_state) = app_state.windows.get_mut(self.window.label()) {
            window_state.tab_ids.push(tab_id.clone());
            window_state.active_tab_id = Some(tab_id.clone());
        }
        app_state.tabs.insert(
            tab_id.clone(),
            crate::state::TabState {
                id: tab_id.clone(),
                title: "Notion Lite".to_string(),
                url: start_url.to_string(),
                is_active: true,
                is_pinned: false,
            },
        );
        crate::state::persist_state(app);
        let _ = tab_manager.show_tab(&tab_id);
        Ok(())
    }
}

fn restored_url(saved_url: &str, start_url: &str) -> String {
    if saved_url.contains("unsupported-browser.html")
        || saved_url.contains("notion.com/en-gb")
        || saved_url == "https://www.notion.com/"
        || saved_url == "https://notion.com/"
    {
        start_url.to_string()
    } else {
        saved_url.to_string()
    }
}

fn restored_title(saved_title: &str) -> String {
    if saved_title.is_empty() || saved_title.contains("Your connected workspace") {
        "Notion".to_string()
    } else {
        saved_title.to_string()
    }
}

/// Invoked when a window is minimized or hidden to the system tray.
/// Suspends heavy UI/rendering JS execution in webviews and compacts OS heap RAM.
pub fn on_window_backgrounded<R: Runtime>(app_handle: &AppHandle<R>, window_label: &str) {
    log::info!(
        "Window {} backgrounded (minimized/hidden) - applying RAM compaction & CPU throttling",
        window_label
    );

    if let Some(app_state_lock) =
        app_handle.try_state::<Arc<std::sync::RwLock<crate::state::AppState>>>()
    {
        if let Ok(mut app_state) = app_state_lock.inner().write() {
            if let Some(w_state) = app_state.windows.get_mut(window_label) {
                w_state.is_minimized = true;
            }
        }
    }

    if let Some(w) = app_handle.get_window(window_label) {
        let script = "if (window.NotionLite && window.NotionLite.onBackground) window.NotionLite.onBackground();";
        for wv in w.webviews() {
            let _ = wv.eval(script);
        }
    }

    compact_process_memory();
}

/// Invoked when a window is unminimized, restored, or brought to focus.
/// Instantly resumes normal webview execution without reloading content.
pub fn on_window_foregrounded<R: Runtime>(app_handle: &AppHandle<R>, window_label: &str) {
    log::info!(
        "Window {} foregrounded (restored/shown) - resuming full activity",
        window_label
    );

    if let Some(app_state_lock) =
        app_handle.try_state::<Arc<std::sync::RwLock<crate::state::AppState>>>()
    {
        if let Ok(mut app_state) = app_state_lock.inner().write() {
            if let Some(w_state) = app_state.windows.get_mut(window_label) {
                w_state.is_minimized = false;
            }
        }
    }

    if let Some(w) = app_handle.get_window(window_label) {
        let script = "if (window.NotionLite && window.NotionLite.onForeground) window.NotionLite.onForeground();";
        for wv in w.webviews() {
            let _ = wv.eval(script);
        }
    }
}

/// Compacts process heap memory when going to background.
pub fn compact_process_memory() {
    #[cfg(target_os = "linux")]
    {
        unsafe {
            extern "C" {
                fn malloc_trim(pad: usize) -> std::os::raw::c_int;
            }
            let trimmed = malloc_trim(0);
            log::debug!(
                "RAM compaction (malloc_trim): freed unused heap memory chunks (result={})",
                trimmed
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{compact_process_memory, restored_title, restored_url};

    #[test]
    fn test_compact_process_memory_runs_safely() {
        compact_process_memory();
    }

    #[test]
    fn restored_url_replaces_dead_pages_with_start_url() {
        let start = "https://app.notion.com";
        for saved in [
            "https://www.notion.com/",
            "https://notion.com/",
            "https://www.notion.com/unsupported-browser.html",
            "https://www.notion.com/en-gb",
        ] {
            assert_eq!(restored_url(saved, start), start, "saved URL: {saved}");
        }
        assert_eq!(
            restored_url("https://www.notion.so/my-page", start),
            "https://www.notion.so/my-page"
        );
    }

    #[test]
    fn restored_title_replaces_blank_and_login_titles() {
        assert_eq!(restored_title(""), "Notion");
        assert_eq!(restored_title("Your connected workspace"), "Notion");
        assert_eq!(restored_title("My Page"), "My Page");
    }
}
