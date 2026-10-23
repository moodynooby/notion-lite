use crate::traits::ThemingEngine;
use std::path::PathBuf;
use std::sync::RwLock;
use tauri::{Runtime, Webview};

/// Normalizes a theme name to one the theme engine knows about.
///
/// `light` is the legacy name for Notion's default (unstyled) theme, so it
/// maps to `default`. Unknown names fall back to `default` instead of being
/// injected blindly into the page.
pub fn normalize_theme_name(theme_name: &str) -> &'static str {
    match theme_name {
        "dracula" => "dracula",
        "nord" => "nord",
        "light" | "default" => "default",
        _ => {
            log::warn!(
                "Unknown theme '{}'; falling back to the default theme",
                theme_name
            );
            "default"
        }
    }
}

/// Notion declares its design tokens on `<body>` (`.notion-dark-theme` /
/// `.notion-light-theme`). Custom properties inherit from the closest
/// ancestor, so a `:root`-only declaration is shadowed for every descendant
/// of `<body>`; the overrides must be repeated on the body-level classes.
const THEME_SCOPE: &str = ":root, :root.dark, :root.light, body, body.notion-body, .notion-dark-theme, .notion-light-theme, .notion-app-inner";

/// Dracula palette. Sources: https://draculatheme.com, mapped onto Notion's
/// `--c-*` design tokens (plus the legacy `--theme--*` set).
const DRACULA_DECLS: &str = r#"
    /* Core backgrounds */
    --c-bacPri: #282a36 !important;
    --c-bacSec: #21222c !important;
    --c-bacTer: #44475a !important;
    --c-bacEle: #21222c !important;
    --c-bacAccPri: #44475a !important;
    --c-bacAccSec: #6272a4 !important;

    /* Borders */
    --c-borPri: #44475a !important;
    --c-borSec: #21222c !important;
    --c-borAccPri: #6272a4 !important;
    --c-borInvPri: #6272a4 !important;
    --c-borStr: #6272a4 !important;

    /* Icons */
    --c-icoPri: #f8f8f2 !important;
    --c-icoSec: #bd93f9 !important;
    --c-icoTer: #6272a4 !important;
    --c-icoAccPri: #bd93f9 !important;
    --c-icoDis: #44475a !important;
    --c-icoInvPri: #f8f8f2 !important;
    --c-icoInvSec: #bd93f9 !important;

    /* Text */
    --c-graTexPri: #f8f8f2 !important;
    --c-graTexSec: #bd93f9 !important;
    --c-graTexTer: #6272a4 !important;
    --c-graTexDis: #44475a !important;
    --c-graTexInvPri: #f8f8f2 !important;
    --c-graTexInvSec: #bd93f9 !important;

    /* Surfaces */
    --c-graBacPri: #21222c !important;
    --c-graBacSec: #44475a !important;
    --c-graBacTer: #6272a4 !important;
    --c-graBacEle: #44475a !important;
    --c-graBacAccPri: #6272a4 !important;
    --c-graBacAccSec: #bd93f9 !important;

    --c-graBorPri: #44475a !important;
    --c-graBorSec: #21222c !important;
    --c-graBorAccPri: #6272a4 !important;
    --c-graBorInvPri: #6272a4 !important;
    --c-graBorStr: #6272a4 !important;

    --c-graIcoPri: #f8f8f2 !important;
    --c-graIcoSec: #bd93f9 !important;
    --c-graIcoTer: #6272a4 !important;
    --c-graIcoAccPri: #bd93f9 !important;
    --c-graIcoDis: #44475a !important;
    --c-graIcoInvPri: #f8f8f2 !important;
    --c-graIcoInvSec: #bd93f9 !important;

    /* Specific surfaces */
    --c-popBac: #21222c !important;
    --c-butGroBac: #44475a !important;
    --c-calIteBac: #44475a !important;
    --c-codStiBloBac: #1e1f29 !important;
    --c-keyActBarBac: #21222c !important;
    --c-keyDonBarBac: #21222c !important;
    --c-peeTimBac: #21222c !important;
    --c-hovMarDisBac: #44475a !important;
    --c-homScrButBacBas: #21222c !important;

    /* Primary action color */
    --c-palUiBlu600: #bd93f9 !important;
    --c-palUiBlu700: #d6acff !important;

    /* Legacy --theme--* variables */
    --theme--bg: #282a36;
    --theme--fg: #44475a;
    --theme--bg-secondary: #21222c;
    --theme--bg-tertiary: #44475a;
    --theme--ui: #44475a;
    --theme--text_ui: #6272a4;
    --theme--text_ui_info: #8be9fd;
    --theme--interactive: #bd93f9;
    --theme--interactive_hover: #ff79c6;
    --theme--divider: #6272a4;
    --theme--ui-border-mode: #44475a;
    --theme--ui-border-image: #44475a;
    --theme--ui-border-table: #44475a;
    --theme--ui-divider: #44475a;
    --theme--ui-input: #21222c;
    --theme--ui-input-border: #6272a4;
    --theme--ui-overlay: rgba(0, 0, 0, 0.4);
    --theme--ui-scrollbar: #44475a;
    --theme--ui-scrollbar-track: #282a36;
    --theme--ui-shadow: rgba(0, 0, 0, 0.5);
    --theme--ui-tooltip: #21222c;
    --theme--ui-tooltip-text: #f8f8f2;
    --theme--text: #f8f8f2;
    --theme--text-secondary: #bd93f9;
    --theme--text-disabled: #6272a4;
    --theme--accent: #bd93f9;
"#;

/// Nord palette. Source: https://www.nordtheme.com.
const NORD_DECLS: &str = r#"
    /* Core backgrounds */
    --c-bacPri: #2e3440 !important;
    --c-bacSec: #3b4252 !important;
    --c-bacTer: #434c5e !important;
    --c-bacEle: #3b4252 !important;
    --c-bacAccPri: #434c5e !important;
    --c-bacAccSec: #4c566a !important;

    /* Borders */
    --c-borPri: #434c5e !important;
    --c-borSec: #3b4252 !important;
    --c-borAccPri: #4c566a !important;
    --c-borInvPri: #4c566a !important;
    --c-borStr: #4c566a !important;

    /* Icons */
    --c-icoPri: #eceff4 !important;
    --c-icoSec: #88c0d0 !important;
    --c-icoTer: #4c566a !important;
    --c-icoAccPri: #88c0d0 !important;
    --c-icoDis: #434c5e !important;
    --c-icoInvPri: #eceff4 !important;
    --c-icoInvSec: #88c0d0 !important;

    /* Text */
    --c-graTexPri: #eceff4 !important;
    --c-graTexSec: #d8dee9 !important;
    --c-graTexTer: #4c566a !important;
    --c-graTexDis: #434c5e !important;
    --c-graTexInvPri: #eceff4 !important;
    --c-graTexInvSec: #88c0d0 !important;

    /* Surfaces */
    --c-graBacPri: #3b4252 !important;
    --c-graBacSec: #434c5e !important;
    --c-graBacTer: #4c566a !important;
    --c-graBacEle: #434c5e !important;
    --c-graBacAccPri: #4c566a !important;
    --c-graBacAccSec: #88c0d0 !important;

    --c-graBorPri: #434c5e !important;
    --c-graBorSec: #3b4252 !important;
    --c-graBorAccPri: #4c566a !important;
    --c-graBorInvPri: #4c566a !important;
    --c-graBorStr: #4c566a !important;

    --c-graIcoPri: #eceff4 !important;
    --c-graIcoSec: #88c0d0 !important;
    --c-graIcoTer: #4c566a !important;
    --c-graIcoAccPri: #88c0d0 !important;
    --c-graIcoDis: #434c5e !important;
    --c-graIcoInvPri: #eceff4 !important;
    --c-graIcoInvSec: #88c0d0 !important;

    /* Specific surfaces */
    --c-popBac: #3b4252 !important;
    --c-butGroBac: #434c5e !important;
    --c-calIteBac: #434c5e !important;
    --c-codStiBloBac: #242933 !important;
    --c-keyActBarBac: #3b4252 !important;
    --c-keyDonBarBac: #3b4252 !important;
    --c-peeTimBac: #3b4252 !important;
    --c-hovMarDisBac: #434c5e !important;
    --c-homScrButBacBas: #3b4252 !important;

    /* Primary action color */
    --c-palUiBlu600: #88c0d0 !important;
    --c-palUiBlu700: #a3d5e0 !important;

    /* Legacy --theme--* variables */
    --theme--bg: #2e3440;
    --theme--fg: #3b4252;
    --theme--bg-secondary: #3b4252;
    --theme--bg-tertiary: #434c5e;
    --theme--ui: #434c5e;
    --theme--text_ui: #4c566a;
    --theme--text_ui_info: #88c0d0;
    --theme--interactive: #81a1c1;
    --theme--interactive_hover: #88c0d0;
    --theme--divider: #4c566a;
    --theme--ui-border-mode: #434c5e;
    --theme--ui-border-image: #434c5e;
    --theme--ui-border-table: #434c5e;
    --theme--ui-divider: #434c5e;
    --theme--ui-input: #2e3440;
    --theme--ui-input-border: #4c566a;
    --theme--ui-overlay: rgba(0, 0, 0, 0.4);
    --theme--ui-scrollbar: #4c566a;
    --theme--ui-scrollbar-track: #2e3440;
    --theme--ui-shadow: rgba(0, 0, 0, 0.5);
    --theme--ui-tooltip: #3b4252;
    --theme--ui-tooltip-text: #eceff4;
    --theme--text: #eceff4;
    --theme--text-secondary: #88c0d0;
    --theme--text-disabled: #4c566a;
    --theme--accent: #88c0d0;
"#;

/// Produces the CSS that overrides Notion's design tokens for a theme.
fn theme_variables(theme_name: &str) -> String {
    let declarations = match normalize_theme_name(theme_name) {
        "dracula" => DRACULA_DECLS,
        "nord" => NORD_DECLS,
        _ => return String::new(),
    };

    format!("{THEME_SCOPE} {{\n{declarations}\n}}")
}

/// Encodes `value` as a JavaScript string literal.
///
/// JSON string syntax is a strict subset of JavaScript string syntax, which
/// makes this a safe way to embed CSS/theme names into the injected scripts.
fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

const STYLE_SCRIPT_TEMPLATE: &str = r#"(function () {
    var styleId = __NL_STYLE_ID__;
    var css = __NL_STYLE_CSS__;
    var apply = function () {
        var root = document.documentElement;
        if (!root) return false;
        var target = document.head || root;
        var style = document.getElementById(styleId);
        if (!style) {
            style = document.createElement('style');
            style.id = styleId;
            target.appendChild(style);
        }
        style.textContent = css;
        return true;
    };
    if (!apply()) document.addEventListener('DOMContentLoaded', apply, { once: true });
})();"#;

const THEME_ATTRIBUTE_TEMPLATE: &str = r#"(function () {
    var name = __NL_THEME_NAME__;
    var apply = function () {
        if (!document.documentElement) return false;
        document.documentElement.setAttribute('data-nl-theme', name);
        return true;
    };
    if (!apply()) document.addEventListener('DOMContentLoaded', apply, { once: true });
})();"#;

/// Builds a document-start-safe script that installs (or updates) a `<style>`
/// element. `document.head` does not exist yet when initialization scripts run,
/// hence the `documentElement` fallback and `DOMContentLoaded` retry.
fn style_script(style_id: &str, css: &str) -> String {
    STYLE_SCRIPT_TEMPLATE
        .replace("__NL_STYLE_ID__", &js_string(style_id))
        .replace("__NL_STYLE_CSS__", &js_string(css))
}

fn theme_attribute_script(theme_name: &str) -> String {
    THEME_ATTRIBUTE_TEMPLATE.replace("__NL_THEME_NAME__", &js_string(theme_name))
}

/// Script appended to every tab's initialization script so the theme is applied
/// on each document load instead of relying on a one-shot `eval()` that is lost
/// as soon as the page navigates.
pub fn theme_init_script(theme_name: &str) -> String {
    let theme = normalize_theme_name(theme_name);
    format!(
        "{}\n{}",
        theme_attribute_script(theme),
        style_script("notion-theme-style", &theme_variables(theme))
    )
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
            active_theme: RwLock::new(normalize_theme_name(theme).to_string()),
            custom_css_path: RwLock::new(css_path),
        }
    }
}

impl<R: Runtime> ThemingEngine<R> for ThemeManager {
    fn get_theme_css(&self, theme_name: &str) -> String {
        let theme = normalize_theme_name(theme_name);
        format!(
            "{}\n{}",
            theme_attribute_script(theme),
            style_script("notion-theme-style", &theme_variables(theme))
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
                        return style_script("notion-custom-style", &css);
                    }
                    Err(e) => log::warn!("Failed to read custom CSS: {}", e),
                }
            }
        }
        String::new()
    }

    fn inject_theme(&self, webview: &Webview<R>, theme_name: &str) {
        let js = <Self as ThemingEngine<R>>::get_theme_css(self, theme_name);
        let _ = webview.eval(&js);

        // Also inject custom CSS if present
        let custom = <Self as ThemingEngine<R>>::get_custom_css(self);
        if !custom.is_empty() {
            let _ = webview.eval(&custom);
        }
    }

    fn set_active_theme(&self, name: &str) {
        let normalized = normalize_theme_name(name);
        let mut theme = self
            .active_theme
            .write()
            .expect("ThemeManager: active_theme write lock poisoned");
        *theme = normalized.to_string();
        log::info!("Active theme changed to: {}", normalized);
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
    fn test_normalize_theme_name() {
        assert_eq!(normalize_theme_name("dracula"), "dracula");
        assert_eq!(normalize_theme_name("nord"), "nord");
        assert_eq!(normalize_theme_name("default"), "default");
        assert_eq!(normalize_theme_name("light"), "default");
        assert_eq!(normalize_theme_name("does-not-exist"), "default");
    }

    #[test]
    fn test_theme_variables_override_notion_tokens() {
        let dracula = theme_variables("dracula");
        assert!(dracula.contains(".notion-dark-theme"));
        assert!(dracula.contains(".notion-light-theme"));
        assert!(dracula.contains("--c-bacPri: #282a36 !important;"));

        let nord = theme_variables("nord");
        assert!(nord.contains("--c-bacPri: #2e3440 !important;"));

        // The default theme must not override anything.
        assert!(theme_variables("default").is_empty());
        assert!(theme_variables("light").is_empty());
    }

    #[test]
    fn test_theme_init_script_is_escaped_and_uses_real_theme() {
        let script = theme_init_script("light");
        assert!(script.contains("data-nl-theme"));
        assert!(script.contains("\"default\""));

        // CSS containing quotes is embedded as a JSON (double-quoted) string
        // literal, so it cannot break out of the injected script. Backticks and
        // `$` are harmless inside a double-quoted literal (unlike template
        // literals, which is what the previous implementation used).
        let script = style_script("notion-custom-style", "body { content: \"a`b$c\"; }");
        assert!(script.contains("\\\"a`b$c\\\""));
        assert!(!script.contains("textContent = body"));
    }
}
