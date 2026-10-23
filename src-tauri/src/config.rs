use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User-facing configuration persisted to disk as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotionConfig {
    pub active_theme: String,
    pub custom_css_path: Option<PathBuf>,
    pub restore_tabs: bool,
    pub show_menu_bar: bool,
    pub start_url: Option<String>,
    pub window: WindowConfig,
}

pub type LotionConfig = NotionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1200.0,
            height: 800.0,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl Default for NotionConfig {
    fn default() -> Self {
        Self {
            active_theme: "dracula".to_string(),
            custom_css_path: None,
            restore_tabs: true,
            show_menu_bar: cfg!(target_os = "macos"),
            start_url: Some("https://app.notion.com".to_string()),
            window: WindowConfig::default(),
        }
    }
}

impl NotionConfig {
    /// Returns the validated start URL, falling back to "https://app.notion.com"
    pub fn get_start_url(&self) -> &str {
        if let Some(ref url) = self.start_url {
            if let Ok(parsed) = url::Url::parse(url) {
                if parsed.scheme() == "https" {
                    if let Some(host) = parsed.host_str() {
                        if crate::policy::is_notion_host(host) {
                            return url.as_str();
                        }
                    }
                }
            }
            log::warn!(
                "Configured start_url '{}' is not a valid Notion HTTPS URL. Falling back to https://app.notion.com.",
                url
            );
        }
        "https://app.notion.com"
    }

    /// Returns the config directory path (~/.config/notion-lite/)
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("notion-lite")
    }

    /// Returns the config file path (~/.config/notion-lite/config.toml)
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load config from disk, or create default if not found.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<NotionConfig>(&contents) {
                    Ok(mut config) => {
                        if let Some(ref path) = config.custom_css_path {
                            let custom_themes_dir = Self::config_dir().join("custom_themes");
                            if !path.starts_with(&custom_themes_dir) {
                                log::warn!("Custom CSS path '{}' is outside the designated custom themes directory. Discarding for security.", path.display());
                                config.custom_css_path = None;
                            } else if !path.extension().is_some_and(|ext| ext == "css") {
                                log::warn!("Custom CSS path '{}' does not have a .css extension. Discarding for security.", path.display());
                                config.custom_css_path = None;
                            } else if !path.exists() {
                                log::warn!(
                                    "Custom CSS path '{}' does not exist. Discarding.",
                                    path.display()
                                );
                                config.custom_css_path = None;
                            }
                        }
                        log::info!("Config loaded from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse config, using defaults: {}", e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read config file, using defaults: {}", e);
                }
            }
        } else {
            log::info!(
                "No config file found, creating default at {}",
                path.display()
            );
        }

        let config = Self::default();
        let _ = config.save(); // Best-effort save of defaults
        config
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;

        let contents = toml::to_string_pretty(self)?;
        fs::write(Self::config_path(), contents)?;

        log::info!("Config saved to {}", Self::config_path().display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_start_url_default_and_custom() {
        let default_config = super::NotionConfig::default();
        assert_eq!(default_config.get_start_url(), "https://app.notion.com");

        // Custom start url on notion.com
        let mut custom_config = default_config.clone();
        custom_config.start_url = Some("https://app.notion.com/my-workspace/my-page".to_string());
        assert_eq!(
            custom_config.get_start_url(),
            "https://app.notion.com/my-workspace/my-page"
        );

        // Custom start url on notion.so
        custom_config.start_url = Some("https://www.notion.so/my-page".to_string());
        assert_eq!(
            custom_config.get_start_url(),
            "https://www.notion.so/my-page"
        );

        // Malicious or invalid start_url falls back safely
        custom_config.start_url = Some("https://evilnotion.com/login".to_string());
        assert_eq!(custom_config.get_start_url(), "https://app.notion.com");

        custom_config.start_url = Some("http://app.notion.com".to_string()); // Insecure HTTP
        assert_eq!(custom_config.get_start_url(), "https://app.notion.com");
    }

    #[test]
    fn test_partial_config_uses_defaults() {
        let config: super::NotionConfig =
            toml::from_str("active_theme = \"nord\"\n").expect("partial config should load");
        assert_eq!(config.active_theme, "nord");
        assert!(config.restore_tabs);
        assert_eq!(config.window.width, 1200.0);
        assert_eq!(config.window.height, 800.0);
        assert_eq!(config.get_start_url(), "https://app.notion.com");

        let window: super::WindowConfig =
            toml::from_str("height = 600.0\n").expect("partial window config should load");
        assert_eq!(window.width, 1200.0);
        assert_eq!(window.height, 600.0);
        assert!(!window.maximized);
    }
}
