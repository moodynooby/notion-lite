use hunspell_rs::Hunspell;
use std::sync::Mutex;
use tauri::State;

pub struct SafeHunspell(pub Hunspell);
unsafe impl Send for SafeHunspell {}
unsafe impl Sync for SafeHunspell {}

pub struct SpellcheckManager {
    pub hunspell: std::sync::Arc<Mutex<Option<SafeHunspell>>>,
}

impl Default for SpellcheckManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SpellcheckManager {
    pub fn new() -> Self {
        let hunspell = std::sync::Arc::new(Mutex::new(None));

        // Cross-platform dictionary discovery
        let (aff, dic) = if cfg!(target_os = "windows") {
            // Common Windows paths or relative to binary
            (
                "C:\\Program Files\\Hunspell\\en_US.aff".to_string(),
                "C:\\Program Files\\Hunspell\\en_US.dic".to_string(),
            )
        } else if cfg!(target_os = "macos") {
            (
                "/Library/Spelling/en_US.aff".to_string(),
                "/Library/Spelling/en_US.dic".to_string(),
            )
        } else {
            (
                "/usr/share/hunspell/en_US.aff".to_string(),
                "/usr/share/hunspell/en_US.dic".to_string(),
            )
        };

        if std::path::Path::new(&aff).exists() && std::path::Path::new(&dic).exists() {
            let hs_clone = hunspell.clone();
            std::thread::spawn(move || {
                let hs = Hunspell::new(&aff, &dic);
                if let Ok(mut lock) = hs_clone.lock() {
                    *lock = Some(SafeHunspell(hs));
                    log::info!("Hunspell dictionaries loaded successfully in background.");
                }
            });
        } else {
            log::warn!(
                "SpellcheckManager: Default en_US dictionaries not found at {}",
                aff
            );
        }

        Self { hunspell }
    }

    pub fn load_dictionaries(&self, aff_path: &str, dic_path: &str) {
        let hs = Hunspell::new(aff_path, dic_path);
        *self
            .hunspell
            .lock()
            .expect("SpellcheckManager: hunspell lock poisoned") = Some(SafeHunspell(hs));
        log::info!("Hunspell dictionaries loaded successfully.");
    }
}

#[tauri::command]
pub fn check_spelling(word: String, state: State<'_, SpellcheckManager>) -> bool {
    let hs_lock = state
        .hunspell
        .lock()
        .expect("SpellcheckManager: hunspell lock poisoned");
    if let Some(hs) = hs_lock.as_ref() {
        matches!(
            hs.0.check(&word),
            hunspell_rs::CheckResult::FoundInDictionary
        )
    } else {
        true
    }
}

#[tauri::command]
pub fn get_spelling_suggestions(word: String, state: State<'_, SpellcheckManager>) -> Vec<String> {
    let hs_lock = state
        .hunspell
        .lock()
        .expect("SpellcheckManager: hunspell lock poisoned");
    if let Some(hs) = hs_lock.as_ref() {
        hs.0.suggest(&word)
    } else {
        Vec::new()
    }
}
