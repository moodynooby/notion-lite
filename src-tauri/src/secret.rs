use rand::RngCore;
use std::fs::{self, File};
use std::io::{Read, Write};
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

const SECRET_FILE_NAME: &str = "secret_key";

/// Loads the app's 32-byte state-encryption secret, creating it if needed.
///
/// Secrets from the legacy `notion` and `lotion-rs` config directories are
/// migrated so existing encrypted sessions keep working.
pub fn get_or_create() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let config_base = dirs::config_dir().ok_or("Could not find config directory")?;
    let secret_dir = config_base.join("notion-lite");
    let secret_path = secret_dir.join(SECRET_FILE_NAME);

    if secret_path.exists() {
        let mut file = File::open(&secret_path)?;
        let mut secret = vec![0u8; 32];
        file.read_exact(&mut secret)?;
        log::info!("Application secret loaded from {}", secret_path.display());
        return Ok(secret);
    }

    // Check for legacy notion or lotion-rs secret key to seamlessly preserve existing sessions
    let legacy_notion_path = config_base.join("notion").join(SECRET_FILE_NAME);
    let legacy_lotion_path = config_base.join("lotion-rs").join(SECRET_FILE_NAME);
    let legacy_path = if legacy_notion_path.exists() {
        Some(legacy_notion_path)
    } else if legacy_lotion_path.exists() {
        Some(legacy_lotion_path)
    } else {
        None
    };

    if let Some(legacy_path) = legacy_path {
        log::info!(
            "Migrating legacy application secret from {}",
            legacy_path.display()
        );
        fs::create_dir_all(&secret_dir)?;
        let mut file = File::open(&legacy_path)?;
        let mut secret = vec![0u8; 32];
        file.read_exact(&mut secret)?;

        let mut new_file = File::create(&secret_path)?;
        #[cfg(target_family = "unix")]
        {
            new_file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        new_file.write_all(&secret)?;
        return Ok(secret);
    }

    log::info!(
        "Generating new application secret at {}",
        secret_path.display()
    );
    fs::create_dir_all(&secret_dir)?;

    let mut secret = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);

    let mut file = File::create(&secret_path)?;
    #[cfg(target_family = "unix")]
    {
        // Set permissions to 0o600 (read/write only for owner)
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(&secret)?;
    Ok(secret)
}
