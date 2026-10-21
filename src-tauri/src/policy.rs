use crate::traits::PolicyEnforcer;

/// True when `host` is Notion or a subdomain of Notion.
///
/// This is the single definition used by navigation policy, start-URL
/// validation, and IPC origin checks so the three cannot drift apart.
pub fn is_notion_host(host: &str) -> bool {
    host == "notion.so"
        || host.ends_with(".notion.so")
        || host == "notion.com"
        || host.ends_with(".notion.com")
}

#[derive(Default)]
pub struct PolicyManager;

impl PolicyManager {
    pub fn new() -> Self {
        Self
    }

    fn is_trusted_oauth(&self, host: &str) -> bool {
        let trusted = ["accounts.google.com", "appleid.apple.com"];
        trusted
            .iter()
            .any(|t| host == *t || host.ends_with(&format!(".{}", t)))
    }
}

impl PolicyEnforcer for PolicyManager {
    fn validate_url(&self, url: &str) -> bool {
        log::debug!("PolicyManager: Validating URL: {}", url);
        if url == "about:blank" {
            return true;
        }
        // Parse URL and check if host is in allowed_domains
        if let Ok(parsed_url) = url::Url::parse(url) {
            if parsed_url.scheme() != "https" {
                log::warn!("PolicyManager: BLOCKED non-HTTPS URL scheme: {}", url);
                return false;
            }

            if let Some(host) = parsed_url.host_str() {
                if is_notion_host(host) {
                    log::debug!("PolicyManager: ALLOWED official Notion domain: {}", host);
                    return true;
                }

                if self.is_trusted_oauth(host) {
                    log::debug!("PolicyManager: ALLOWED OAuth provider: {}", host);
                    return true;
                }

                log::warn!("PolicyManager: BLOCKED unauthorized endpoint: {}", host);
                return false;
            }
        }

        log::warn!(
            "PolicyManager: BLOCKED malformed or unsupported URL: {}",
            url
        );
        // Block other protocols/malformed URLs by default (Zero-Trust)
        false
    }

    fn telemetry_allowed(&self) -> bool {
        // Manifesto Part III: Anti-Telemetry by Default
        false
    }

    fn validate_external_link(&self, url: &str) -> bool {
        // For external links, we apply a broader security check.
        // For now, allow https only and block common malicious protocols.
        if let Ok(parsed_url) = url::Url::parse(url) {
            // Block known tracker and telemetry domains from jumping out to the browser
            if let Some(host) = parsed_url.host_str() {
                let trackers = [
                    "googletagmanager.com",
                    "google-analytics.com",
                    "amplitude.com",
                    "mixpanel.com",
                    "segment.com",
                ];
                if trackers
                    .iter()
                    .any(|t| host == *t || host.ends_with(&format!(".{}", t)))
                {
                    log::warn!("PolicyManager: BLOCKED tracker/telemetry domain: {}", host);
                    return false;
                }
            }

            match parsed_url.scheme() {
                "https" => true,
                "mailto" => true,
                _ => {
                    log::warn!(
                        "PolicyManager: BLOCKED unsafe external link protocol: {}",
                        parsed_url.scheme()
                    );
                    false
                }
            }
        } else {
            false
        }
    }

    fn should_route_popup_to_system_browser(&self, url: &str) -> bool {
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                // If it's an official Notion domain or OAuth provider, keep it in the app
                if is_notion_host(host) || self.is_trusted_oauth(host) {
                    return false;
                }
            }

            // Local tauri schemes stay in-app
            if parsed_url.scheme() == "tauri" {
                return false;
            }
        }

        // Everything else is an external link and should open in system browser
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PolicyEnforcer;

    #[test]
    fn test_should_route_popup_to_system_browser() {
        let policy = PolicyManager::new();
        // OAuth providers should NOT be routed (they stay in app to capture session)
        assert!(!policy
            .should_route_popup_to_system_browser("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(!policy.should_route_popup_to_system_browser("https://appleid.apple.com/auth"));

        // External links (e.g., GitHub, Slack) should be routed to system browser
        assert!(policy.should_route_popup_to_system_browser("https://github.com/login"));
        assert!(policy.should_route_popup_to_system_browser("https://slack.com/signin"));

        // Notion internal popups should NOT be routed
        assert!(!policy.should_route_popup_to_system_browser("https://www.notion.so/some-popup"));
        assert!(!policy.should_route_popup_to_system_browser("https://www.notion.com/some-popup"));

        // Local tauri schemes should NOT be routed
        assert!(!policy.should_route_popup_to_system_browser("tauri://localhost/index.html"));
    }

    #[test]
    fn test_validate_official_notion_urls() {
        let policy = PolicyManager::new();
        assert!(policy.validate_url("https://www.notion.so"));
        assert!(policy.validate_url("https://www.notion.so/login"));
        assert!(policy.validate_url("https://notion.com/some-page"));
        assert!(policy.validate_url("https://app.notion.com"));
        assert!(policy.validate_url("https://app.notion.com/workspace-name"));
        assert!(policy.validate_url("https://msgstore.www.notion.so/v1/health"));
        assert!(policy.validate_url("https://msgstore.www.notion.com/v1/health"));

        // OAuth providers allowed during login
        assert!(policy.validate_url("https://accounts.google.com/auth"));
        assert!(policy.validate_url("https://appleid.apple.com/auth"));
    }

    #[test]
    fn test_block_unauthorized_urls() {
        let policy = PolicyManager::new();
        assert!(!policy.validate_url("https://google.com"));
        assert!(!policy.validate_url("https://facebook.com"));
        assert!(!policy.validate_url("https://malicious-site.com"));

        // Zero-Trust: Subdomain/Suffix Attacks
        assert!(!policy.validate_url("https://evilnotion.so"));
        assert!(!policy.validate_url("https://notion.so.evil.com"));
        assert!(!policy.validate_url("https://accounts.google.com.evil.com"));
        assert!(!policy.validate_url("https://malicious-site.com/notion.so"));

        // Protocol security
        assert!(!policy.validate_url("http://www.notion.so")); // Enforce HTTPS
        assert!(!policy.validate_url("javascript:alert(1)"));
        assert!(!policy.validate_url("data:text/html,base64..."));
    }

    #[test]
    fn test_telemetry_always_blocked() {
        let policy = PolicyManager::new();
        assert!(!policy.telemetry_allowed());
    }

    #[test]
    fn test_validate_external_links() {
        let policy = PolicyManager::new();
        // Repository and support links should work (they open in browser)
        assert!(policy.validate_external_link("https://github.com/moodynooby/notion-lite"));
        assert!(policy.validate_external_link("https://github.com/puneetsl/lotion"));
        assert!(policy.validate_external_link("mailto:support@notion.so"));

        // Block unsafe protocols for external browser redirection
        assert!(!policy.validate_external_link("http://unsecure-link.com"));
        assert!(!policy.validate_external_link("javascript:alert('XSS')"));
        assert!(!policy.validate_external_link("file:///etc/passwd"));

        // Block trackers/analytics even for external clicks
        assert!(!policy.validate_external_link("https://www.googletagmanager.com/gtm.js"));
        assert!(!policy.validate_external_link("https://www.google-analytics.com/collect"));
    }

    #[test]
    fn test_suffix_attacks() {
        let policy = PolicyManager::new();
        // POC: evilnotion.so should NOT match notion.so
        assert!(!policy.validate_url("https://evilnotion.so"));
        assert!(!policy.validate_url("https://accounts.google.com.evil.com"));
        assert!(!policy.validate_url("https://appleid.apple.com.attacker.com"));
    }

    #[test]
    fn test_is_notion_host_matches_only_notion_subdomains() {
        assert!(is_notion_host("notion.so"));
        assert!(is_notion_host("www.notion.so"));
        assert!(is_notion_host("file.notion.so"));
        assert!(is_notion_host("notion.com"));
        assert!(is_notion_host("app.notion.com"));

        assert!(!is_notion_host("evilnotion.com"));
        assert!(!is_notion_host("notion.com.evil.com"));
        assert!(!is_notion_host("notioncom"));
        assert!(!is_notion_host(""));
    }
}
