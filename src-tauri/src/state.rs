use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Manager, Runtime};

use crate::crypto::{
    decrypt_data, encrypt_data, get_encryption_key, get_encryption_key_for_salt, EncryptedState,
};

static STATE_DIRTY_VERSION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub id: String,
    pub title: String,
    pub url: String,
    pub is_active: bool,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub id: String,
    pub bounds: Bounds,
    pub is_focused: bool,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub is_full_screen: bool,
    pub tab_ids: Vec<String>,
    pub active_tab_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub windows: HashMap<String, WindowState>,
    pub tabs: HashMap<String, TabState>,
    pub focused_window_id: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            tabs: HashMap::new(),
            focused_window_id: None,
        }
    }

    /// Returns the state file path (~/.config/notion-lite/state.json)
    fn state_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("notion-lite")
            .join("state.json")
    }

    /// Save application state to disk.
    pub fn save_to_disk(&self, app_secret: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let key = get_encryption_key(app_secret);
        let plaintext = serde_json::to_string(self)?;
        let (ciphertext, nonce) =
            encrypt_data(plaintext.as_bytes(), &key).map_err(Box::<dyn std::error::Error>::from)?;

        let encrypted_state = EncryptedState {
            data: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(&nonce),
        };
        let json = serde_json::to_string(&encrypted_state)?;

        std::fs::write(&path, json)?;
        log::info!("Encrypted AppState saved to {}", path.display());
        Ok(())
    }

    /// Load application state from disk.
    pub fn load_from_disk(app_secret: &[u8]) -> Option<Self> {
        let mut path = Self::state_path();
        if !path.exists() {
            // Check legacy notion and lotion-rs paths
            let legacy_notion_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("notion")
                .join("state.json");
            let legacy_lotion_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("lotion-rs")
                .join("state.json");
            if legacy_notion_path.exists() {
                path = legacy_notion_path;
            } else if legacy_lotion_path.exists() {
                path = legacy_lotion_path;
            }
        }

        if path.exists() {
            let key = get_encryption_key(app_secret);
            match std::fs::read_to_string(&path) {
                Ok(contents) => {
                    // Try to load as encrypted state first
                    if let Ok(encrypted_state) = serde_json::from_str::<EncryptedState>(&contents) {
                        let decoded_data =
                            match general_purpose::STANDARD.decode(&encrypted_state.data) {
                                Ok(d) => d,
                                Err(e) => {
                                    log::warn!("Failed to base64 decode encrypted data: {}", e);
                                    return None;
                                }
                            };
                        let decoded_nonce =
                            match general_purpose::STANDARD.decode(&encrypted_state.nonce) {
                                Ok(n) => n,
                                Err(e) => {
                                    log::warn!("Failed to base64 decode nonce: {}", e);
                                    return None;
                                }
                            };

                        // First try with notion-lite key, fallback to notion or legacy lotion-rs key if needed
                        let decrypt_res = decrypt_data(&decoded_data, &decoded_nonce, &key)
                            .or_else(|_| {
                                let notion_key = get_encryption_key_for_salt(app_secret, "notion");
                                decrypt_data(&decoded_data, &decoded_nonce, &notion_key)
                            })
                            .or_else(|_| {
                                let legacy_key =
                                    get_encryption_key_for_salt(app_secret, "lotion-rs");
                                decrypt_data(&decoded_data, &decoded_nonce, &legacy_key)
                            });

                        match decrypt_res {
                            Ok(plaintext_bytes) => match String::from_utf8(plaintext_bytes) {
                                Ok(plaintext) => match serde_json::from_str::<AppState>(&plaintext)
                                {
                                    Ok(state) => {
                                        log::info!(
                                            "Encrypted AppState loaded from {}",
                                            path.display()
                                        );
                                        return Some(state);
                                    }
                                    Err(e) => log::warn!("Failed to parse decrypted state: {}", e),
                                },
                                Err(e) => {
                                    log::warn!("Failed to convert decrypted bytes to string: {}", e)
                                }
                            },
                            Err(e) => log::warn!("Failed to decrypt state file: {}", e),
                        }
                    }

                    // If encrypted load failed, try to load as plaintext (for backward compatibility)
                    if let Ok(state) = serde_json::from_str::<AppState>(&contents) {
                        log::warn!(
                            "Loaded unencrypted AppState from {}. Please re-save to encrypt.",
                            path.display()
                        );
                        return Some(state);
                    }
                    log::warn!("Failed to load state file as either encrypted or plaintext.");
                }
                Err(e) => log::warn!("Failed to read state file: {}", e),
            }
        }
        None
    }
}

/// Persist the shared app state asynchronously with debouncing.
///
/// Rapid window/tab mutations are coalesced so that state is not encrypted
/// and written to disk continuously. The actual encryption and disk I/O run
/// on a worker thread pool via `spawn_blocking`.
///
/// Rather than eagerly cloning `AppState` on every state change, this function
/// reads the latest `AppState` through a read lock inside `spawn_blocking` only
/// after the debounce delay completes.
pub fn persist_state<R: Runtime>(app: &AppHandle<R>) {
    let secret = match app.try_state::<std::sync::Arc<Vec<u8>>>() {
        Some(s) => s.inner().clone(),
        None => {
            log::error!("Zero-Trust: application secret missing; state not saved");
            return;
        }
    };
    let state_lock = match app.try_state::<std::sync::Arc<std::sync::RwLock<AppState>>>() {
        Some(s) => s.inner().clone(),
        None => {
            log::error!("AppState state not managed; state not saved");
            return;
        }
    };
    let current_ver = STATE_DIRTY_VERSION.fetch_add(1, Ordering::SeqCst) + 1;

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // Only write if no newer update has superseded this one
        if STATE_DIRTY_VERSION.load(Ordering::SeqCst) == current_ver {
            tauri::async_runtime::spawn_blocking(move || {
                let app_state = match state_lock.read() {
                    Ok(guard) => guard,
                    Err(e) => {
                        log::error!("AppState read lock poisoned during save: {}", e);
                        return;
                    }
                };
                if let Err(e) = app_state.save_to_disk(&secret) {
                    log::error!("Failed to save application state: {}", e);
                }
            });
        }
    });
}

/// Immediately and synchronously flushes the shared app state to disk.
///
/// Use this on shutdown or window close events where async tasks may not complete.
pub fn persist_state_now<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    match app.try_state::<std::sync::Arc<Vec<u8>>>() {
        Some(secret) => {
            STATE_DIRTY_VERSION.fetch_add(1, Ordering::SeqCst);
            if let Err(e) = state.save_to_disk(secret.inner().as_slice()) {
                log::error!("Failed to synchronously save application state: {}", e);
            }
        }
        None => log::error!("Zero-Trust: application secret missing; state not saved"),
    }
}
