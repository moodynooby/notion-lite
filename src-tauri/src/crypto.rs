use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::StdRng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::OnceLock;

const PBKDF2_ITERATIONS: u32 = 100_000;

static CACHED_KEY: OnceLock<Key<Aes256Gcm>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EncryptedState {
    pub(crate) data: String,  // Base64 encoded encrypted data
    pub(crate) nonce: String, // Base64 encoded nonce
}

pub(crate) fn get_encryption_key_for_salt(app_secret: &[u8], app_dir_name: &str) -> Key<Aes256Gcm> {
    let salt_source = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(app_dir_name)
        .to_string_lossy()
        .into_owned();

    let mut key_bytes = Key::<Aes256Gcm>::default();
    pbkdf2_hmac::<Sha256>(
        app_secret,
        salt_source.as_bytes(),
        PBKDF2_ITERATIONS,
        &mut key_bytes,
    );
    key_bytes
}

pub(crate) fn get_encryption_key(app_secret: &[u8]) -> Key<Aes256Gcm> {
    *CACHED_KEY.get_or_init(|| get_encryption_key_for_salt(app_secret, "notion-lite"))
}

pub(crate) fn encrypt_data(
    data: &[u8],
    key: &Key<Aes256Gcm>,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let cipher = Aes256Gcm::new(key);
    let mut rng = StdRng::from_entropy();
    let mut nonce_bytes = vec![0u8; 12]; // GCM nonces are 12 bytes
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    cipher
        .encrypt(nonce, data)
        .map(|cipher_text| (cipher_text, nonce_bytes))
        .map_err(|e| format!("Encryption error: {:?}", e))
}

pub(crate) fn decrypt_data(
    encrypted_data: &[u8],
    nonce_bytes: &[u8],
    key: &Key<Aes256Gcm>,
) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, encrypted_data)
        .map_err(|e| format!("Decryption error: {:?}", e))
}
