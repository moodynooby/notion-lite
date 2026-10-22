use crate::traits::ThemingEngine;
use std::path::PathBuf;
use std::sync::RwLock;
use tauri::{Runtime, Webview};

pub fn escape_css_for_js_template(css: &str) -> String {
    css.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('$', "\\$")
}

pub struct ThemeManager {
    active_theme: RwLock<String>,
    custom_css_path: RwLock<Option<PathBuf>>,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeManager {
    pub fn new() -> Self {
        Self {
            active_theme: RwLock::new("dracula".to_string()),
            custom_css_path: RwLock::new(None),
        }
    }

    pub fn with_config(theme: &str, css_path: Option<PathBuf>) -> Self {
        Self {
            active_theme: RwLock::new(theme.to_string()),
            custom_css_path: RwLock::new(css_path),
        }
    }

    fn generate_notion_variables(&self, theme_name: &str) -> String {
        match theme_name {
            "dracula" => r#"
                :root.dark {
                    --theme--bg: #282a36;
                    --theme--fg: #44475a;
                    --theme--text: #f8f8f2;
                    --theme--text_ui: #6272a4;
                    --theme--text_ui_info: #8be9fd;
                    --theme--interactive: #bd93f9;
                    --theme--interactive_hover: #ff79c6;
                    --theme--divider: #6272a4;
                    color: #f8f8f2 !important;
                }
                :root.dark .notion-sidebar, :root.dark .notion-topbar {
                    background: #21222c !important;
                    color: #f8f8f2 !important;
                }
            "#
            .to_string(),
            "nord" => r#"
                :root.dark {
                    --theme--bg: #2e3440;
                    --theme--fg: #3b4252;
                    --theme--text: #eceff4;
                    --theme--text_ui: #4c566a;
                    --theme--text_ui_info: #88c0d0;
                    --theme--interactive: #81a1c1;
                    --theme--interactive_hover: #88c0d0;
                    --theme--divider: #4c566a;
                    color: #eceff4 !important;
                }
                :root.dark .notion-sidebar, :root.dark .notion-topbar {
                    background: #2e3440 !important;
                    color: #eceff4 !important;
                }
            "#
            .to_string(),
            "default" | "light" => "".to_string(),
            _ => "".to_string(),
        }
    }
}

impl<R: Runtime> ThemingEngine<R> for ThemeManager {
    fn get_theme_css(&self, theme_name: &str) -> String {
        let vars = self.generate_notion_variables(theme_name);
        format!(
            "(function() {{
            document.documentElement.setAttribute('data-nl-theme', '{}');
            const style = document.getElementById('notion-theme-style') || document.createElement('style');
            style.id = 'notion-theme-style';
            style.textContent = `{}`;
            if (!style.parentElement) document.head.appendChild(style);
        }})();",
            theme_name, vars
        )
    }

    fn get_custom_css(&self) -> String {
        let path_guard = self
            .custom_css_path
            .read()
            .expect("ThemeManager: custom_css_path read lock poisoned");
        if let Some(path) = path_guard.as_ref() {
            if path.exists() {
                match std::fs::read_to_string(path) {
                    Ok(css) => {
                        log::info!("Loaded custom CSS from {}", path.display());
                        // Escape backslashes, backticks, and dollar signs to prevent JavaScript template injection/syntax errors
                        let escaped_css = escape_css_for_js_template(&css);
                        return format!("(function() {{
                            const style = document.getElementById('notion-custom-style') || document.createElement('style');
                            style.id = 'notion-custom-style';
                            style.textContent = `{}`;
                            if (!style.parentElement) document.head.appendChild(style);
                        }})();", escaped_css);
                    }
                    Err(e) => log::warn!("Failed to read custom CSS: {}", e),
                }
            }
        }
        "".to_string()
    }

    fn inject_theme(&self, webview: &Webview<R>, theme_name: &str) {
        let js = <Self as ThemingEngine<R>>::get_theme_css(self, theme_name);
        if !js.is_empty() {
            let _ = webview.eval(&js);
        }
        // Also inject custom CSS if present
        let custom = <Self as ThemingEngine<R>>::get_custom_css(self);
        if !custom.is_empty() {
            let _ = webview.eval(&custom);
        }
    }

    fn set_active_theme(&self, name: &str) {
        let mut theme = self
            .active_theme
            .write()
            .expect("ThemeManager: active_theme write lock poisoned");
        *theme = name.to_string();
        log::info!("Active theme changed to: {}", name);
    }

    fn get_active_theme(&self) -> String {
        self.active_theme
            .read()
            .expect("ThemeManager: active_theme read lock poisoned")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_css_for_js_template() {
        // Backslashes must be escaped
        assert_eq!(
            escape_css_for_js_template("content: '\\2022'"),
            "content: '\\\\2022'"
        );
        // Backticks must be escaped without doubling the backslash
        assert_eq!(
            escape_css_for_js_template("body { font-family: `Consolas`; }"),
            "body { font-family: \\`Consolas\\`; }"
        );
        // Mixed backslashes and backticks
        assert_eq!(escape_css_for_js_template("a\\`b"), "a\\\\\\`b");
        // Dollar signs must be escaped to prevent ${...} interpolation in template literals
        assert_eq!(
            escape_css_for_js_template("calc(${var} * 2)"),
            "calc(\\${var} * 2)"
        );
    }
}
