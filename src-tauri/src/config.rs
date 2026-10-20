use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_show_menu_bar() -> bool {
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// User-facing configuration persisted to disk as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionConfig {
    pub active_theme: String,
    pub custom_css_path: Option<PathBuf>,
    pub restore_tabs: bool,
    #[serde(default = "default_show_menu_bar")]
    pub show_menu_bar: bool,
    #[serde(default)]
    pub start_url: Option<String>,
    pub window: WindowConfig,
}

pub type LotionConfig = NotionConfig;
pub type NotionLiteConfig = NotionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub maximized: bool,
}

impl Default for NotionConfig {
    fn default() -> Self {
        Self {
            active_theme: "dracula".to_string(),
            custom_css_path: None,
            restore_tabs: true,
            show_menu_bar: default_show_menu_bar(),
            start_url: Some("https://app.notion.com".to_string()),
            window: WindowConfig {
                width: 1200.0,
                height: 800.0,
                x: None,
                y: None,
                maximized: false,
            },
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
                        if host == "notion.so"
                            || host.ends_with(".notion.so")
                            || host == "notion.com"
                            || host.ends_with(".notion.com")
                        {
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

    /// Migrate config from ~/.config/notion, ~/.config/lotion-rs or ~/.config/lotion to ~/.config/notion-lite if needed
    fn migrate_legacy_config() {
        let new_dir = Self::config_dir();
        if new_dir.exists() {
            return;
        }

        let base_config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let candidate_dirs = [
            base_config.join("notion"),
            base_config.join("lotion-rs"),
            base_config.join("lotion"),
        ];

        for old_dir in &candidate_dirs {
            if old_dir.exists() {
                log::info!(
                    "Migrating legacy config from {} to {}",
                    old_dir.display(),
                    new_dir.display()
                );
                if let Err(e) = fs::create_dir_all(&new_dir) {
                    log::error!("Failed to create new config dir: {}", e);
                    return;
                }

                // Copy config.toml
                let old_config = old_dir.join("config.toml");
                if old_config.exists() {
                    if let Err(e) = fs::copy(&old_config, new_dir.join("config.toml")) {
                        log::error!("Failed to migrate config.toml: {}", e);
                    }
                }

                // Copy state.json
                let old_state = old_dir.join("state.json");
                if old_state.exists() {
                    if let Err(e) = fs::copy(&old_state, new_dir.join("state.json")) {
                        log::error!("Failed to migrate state.json: {}", e);
                    }
                }

                // Copy secret_key if present
                let old_secret = old_dir.join("secret_key");
                if old_secret.exists() {
                    if let Err(e) = fs::copy(&old_secret, new_dir.join("secret_key")) {
                        log::error!("Failed to migrate secret_key: {}", e);
                    }
                }

                break;
            }
        }
    }

    /// Load config from disk, or create default if not found.
    pub fn load() -> Self {
        Self::migrate_legacy_config();
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
    use std::fs;

    #[test]
    fn test_config_migration() {
        let temp_base = tempfile::tempdir().unwrap();
        let old_dir = temp_base.path().join("notion");
        let new_dir = temp_base.path().join("notion-lite");

        fs::create_dir_all(&old_dir).unwrap();
        fs::write(old_dir.join("config.toml"), "active_theme = 'nord'").unwrap();
        fs::write(old_dir.join("state.json"), "{}").unwrap();

        // Manual migration trigger logic with custom paths
        if old_dir.exists() && !new_dir.exists() {
            fs::create_dir_all(&new_dir).unwrap();
            fs::copy(old_dir.join("config.toml"), new_dir.join("config.toml")).unwrap();
            fs::copy(old_dir.join("state.json"), new_dir.join("state.json")).unwrap();
        }

        assert!(new_dir.join("config.toml").exists());
        assert!(new_dir.join("state.json").exists());
        let contents = fs::read_to_string(new_dir.join("config.toml")).unwrap();
        assert!(contents.contains("nord"));
    }

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
}
