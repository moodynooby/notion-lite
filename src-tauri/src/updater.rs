use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

/// Checks for an update, downloads and installs it, then restarts the app.
///
/// Returns `Ok(false)` when the app is already up to date. A successful
/// install restarts the process, so this future does not return after an
/// update is applied.
pub async fn check_for_updates<R: Runtime>(
    app: AppHandle<R>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    log::info!("Updater: Checking for updates from endpoint...");
    let updater = app.updater()?;
    if let Some(update) = updater.check().await? {
        log::info!(
            "Updater: Update available! Version {} -> {}",
            update.current_version,
            update.version
        );
        let mut downloaded: usize = 0;
        update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    log::info!("Updater: Downloaded {} of {:?}", downloaded, content_length);
                },
                || {
                    log::info!("Updater: Download complete, applying update...");
                },
            )
            .await?;
        log::info!("Updater: Update installed successfully. Relaunching application...");
        app.restart();
    } else {
        log::info!("Updater: Application is up to date.");
        Ok(false)
    }
}
