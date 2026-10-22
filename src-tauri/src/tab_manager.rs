use crate::tab_controller::TabController;
use crate::traits::TabOrchestrator;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager, Runtime};

impl<R: Runtime> TabOrchestrator<R> for TabManager<R> {
    fn create_tab(&self, app: &AppHandle<R>, window_id: &str, url: &str) -> tauri::Result<String> {
        self.create_tab(app, window_id, url)
    }

    fn register_hibernated_tab(&self, window_id: &str, tab_id: String, url: &str) {
        self.register_hibernated_tab(window_id, tab_id, url)
    }

    fn set_app_handle(&self, app: AppHandle<R>) {
        self.set_app_handle(app);
    }

    fn destroy_tab(&self, tab_id: &str) -> tauri::Result<()> {
        self.destroy_tab(tab_id)
    }

    fn show_tab(&self, tab_id: &str) -> tauri::Result<()> {
        self.show_tab(tab_id)
    }

    fn get_tab_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .tabs
            .read()
            .expect("TabManager: tabs read lock poisoned")
            .keys()
            .cloned()
            .collect();
        let hibernated = self
            .hibernated_tabs
            .read()
            .expect("TabManager: hibernated_tabs read lock poisoned");
        for k in hibernated.keys() {
            if !ids.contains(k) {
                ids.push(k.clone());
            }
        }
        ids
    }

    fn inject_theme_into_tab(
        &self,
        app: &AppHandle<R>,
        tab_id: &str,
        theme_name: &str,
    ) -> tauri::Result<()> {
        if let Some(tab) = self
            .tabs
            .read()
            .expect("TabManager: tabs read lock poisoned")
            .get(tab_id)
        {
            let theming = app.state::<Arc<dyn crate::traits::ThemingEngine<R>>>();
            theming.inject_theme(&tab.webview, theme_name);
        }
        Ok(())
    }
}

pub struct TabManager<R: Runtime> {
    pub tabs: RwLock<HashMap<String, Arc<TabController<R>>>>,
    pub hibernated_tabs: RwLock<HashMap<String, (String, String)>>,
    pub active_tab_id: RwLock<Option<String>>,
    pub app_handle: RwLock<Option<AppHandle<R>>>,
}

impl<R: Runtime> Default for TabManager<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runtime> TabManager<R> {
    pub fn new() -> Self {
        Self {
            tabs: RwLock::new(HashMap::new()),
            hibernated_tabs: RwLock::new(HashMap::new()),
            active_tab_id: RwLock::new(None),
            app_handle: RwLock::new(None),
        }
    }

    pub fn set_app_handle(&self, app: AppHandle<R>) {
        let mut handle = self
            .app_handle
            .write()
            .expect("TabManager: app_handle write lock poisoned");
        *handle = Some(app);
    }

    pub fn register_hibernated_tab(&self, window_id: &str, tab_id: String, url: &str) {
        log::info!(
            "TabManager: Registered hibernated tab {} for url {}",
            tab_id,
            url
        );
        let mut hibernated = self
            .hibernated_tabs
            .write()
            .expect("TabManager: hibernated_tabs write lock poisoned");
        hibernated.insert(tab_id, (window_id.to_string(), url.to_string()));
    }

    pub fn create_tab(
        &self,
        app: &AppHandle<R>,
        window_id: &str,
        url: &str,
    ) -> tauri::Result<String> {
        let tab_id = uuid::Uuid::new_v4().to_string();
        self.create_tab_with_id(app, window_id, tab_id, url)
    }

    pub fn create_tab_with_id(
        &self,
        app: &AppHandle<R>,
        window_id: &str,
        tab_id: String,
        url: &str,
    ) -> tauri::Result<String> {
        self.set_app_handle(app.clone());
        let tab_controller = TabController::new(app, window_id, tab_id.clone(), url)?;

        // If tab was previously hibernated, remove from hibernated list
        self.hibernated_tabs
            .write()
            .expect("TabManager: hibernated_tabs write lock poisoned")
            .remove(&tab_id);

        self.tabs
            .write()
            .expect("TabManager: tabs write lock poisoned")
            .insert(tab_id.clone(), Arc::new(tab_controller));

        log::info!("TabManager: Created tab {}", tab_id);
        Ok(tab_id)
    }

    pub fn show_tab(&self, tab_id: &str) -> tauri::Result<()> {
        // Check if tab needs to be awakened from hibernation
        let hibernated_info = self
            .hibernated_tabs
            .read()
            .expect("TabManager: hibernated_tabs read lock poisoned")
            .get(tab_id)
            .cloned();

        if let Some((window_id, url)) = hibernated_info {
            let app_opt = self
                .app_handle
                .read()
                .expect("TabManager: app_handle read lock poisoned")
                .clone();
            if let Some(app) = app_opt {
                log::info!("TabManager: Awakening hibernated tab {}", tab_id);
                self.create_tab_with_id(&app, &window_id, tab_id.to_string(), &url)?;
            } else {
                log::warn!(
                    "TabManager: Cannot awaken hibernated tab {}; AppHandle not set",
                    tab_id
                );
            }
        }

        // Targeted O(1) tab switching: only hide previously active, show new
        let mut active_lock = self
            .active_tab_id
            .write()
            .expect("TabManager: active_tab_id write lock poisoned");
        let old_active = active_lock.take();
        *active_lock = Some(tab_id.to_string());
        drop(active_lock);

        let tabs = self
            .tabs
            .read()
            .expect("TabManager: tabs read lock poisoned");

        if let Some(old_id) = old_active {
            if old_id != tab_id {
                if let Some(old_tab) = tabs.get(&old_id) {
                    let _ = old_tab.hide();
                }
            }
        }

        if let Some(tab) = tabs.get(tab_id) {
            tab.show()?;
        }

        Ok(())
    }

    pub fn destroy_tab(&self, tab_id: &str) -> tauri::Result<()> {
        self.hibernated_tabs
            .write()
            .expect("TabManager: hibernated_tabs write lock poisoned")
            .remove(tab_id);

        let mut active_lock = self
            .active_tab_id
            .write()
            .expect("TabManager: active_tab_id write lock poisoned");
        if active_lock.as_deref() == Some(tab_id) {
            *active_lock = None;
        }
        drop(active_lock);

        if let Some(tab) = self
            .tabs
            .write()
            .expect("TabManager: tabs write lock poisoned")
            .remove(tab_id)
        {
            tab.destroy()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hibernated_tab_lifecycle() {
        let manager = TabManager::<tauri::Wry>::new();
        let tab_id = "test-tab-1".to_string();
        let url = "https://app.notion.com/page-1";

        manager.register_hibernated_tab("main", tab_id.clone(), url);

        let ids = manager.get_tab_ids();
        assert!(ids.contains(&tab_id));

        // Hibernated tab can be destroyed cleanly without panicking
        assert!(manager.destroy_tab(&tab_id).is_ok());
        let ids_after = manager.get_tab_ids();
        assert!(!ids_after.contains(&tab_id));
    }
}
