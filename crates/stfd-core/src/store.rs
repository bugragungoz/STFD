//! The files STFD keeps: `apps.json` and `settings.json` in the config folder, `history.json`
//! in the data folder. Writes go to a temporary file first and replace the old one, so a crash
//! never leaves half a file; a file that cannot be read is set aside, not lost.

use crate::model::{App, HistoryEntry, Settings};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// How many sessions the history keeps.
pub const HISTORY_MAX: usize = 300;

pub struct Store {
    config: PathBuf,
    data: PathBuf,
}

impl Store {
    pub fn new(config: impl Into<PathBuf>, data: impl Into<PathBuf>) -> Self {
        Self {
            config: config.into(),
            data: data.into(),
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn apps(&self) -> Vec<App> {
        read(&self.config.join("apps.json"))
    }

    pub fn save_apps(&self, apps: &[App]) -> io::Result<()> {
        write(&self.config.join("apps.json"), &apps)
    }

    pub fn settings(&self) -> Settings {
        read::<Settings>(&self.config.join("settings.json")).sanitized()
    }

    pub fn save_settings(&self, settings: &Settings) -> io::Result<()> {
        write(&self.config.join("settings.json"), settings)
    }

    /// Newest first.
    pub fn history(&self) -> Vec<HistoryEntry> {
        read(&self.data.join("history.json"))
    }

    pub fn add_history(&self, entry: HistoryEntry) -> io::Result<Vec<HistoryEntry>> {
        let mut all = self.history();
        all.insert(0, entry);
        all.truncate(HISTORY_MAX);
        write(&self.data.join("history.json"), &all)?;
        Ok(all)
    }

    pub fn clear_history(&self) -> io::Result<()> {
        write(&self.data.join("history.json"), &Vec::<HistoryEntry>::new())
    }
}

/// Reads a JSON file; a missing file gives the default, an unreadable one is renamed to `.bad`
/// (kept for a look) and gives the default.
pub fn read<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => {
            let _ = fs::rename(path, path.with_extension("json.bad"));
            T::default()
        }
    }
}

/// Writes JSON to `path.tmp`, then replaces `path` with it.
pub fn write<T: Serialize + ?Sized>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Outcome, Trigger};

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stfd-store-{}", crate::new_id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(id: &str) -> HistoryEntry {
        HistoryEntry {
            id: id.into(),
            app_id: "a".into(),
            app_name: "A".into(),
            trigger: Trigger::Shortcut,
            outcome: Outcome::Cleaned,
            opened_at: Some(1),
            ended_at: 2,
            started: vec![],
            stopped: vec!["S".into()],
            closed: vec![],
            ended: vec![],
            problems: vec![],
        }
    }

    #[test]
    fn missing_files_give_defaults_and_saved_ones_come_back() {
        let dir = temp();
        let store = Store::new(dir.join("config"), dir.join("data"));
        assert!(store.apps().is_empty());
        assert_eq!(store.settings(), Settings::default());

        let settings = Settings {
            wait_seconds: 5,
            ..Settings::default()
        };
        store.save_settings(&settings).unwrap();
        assert_eq!(store.settings(), settings);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn history_is_newest_first_and_capped() {
        let dir = temp();
        let store = Store::new(dir.join("config"), dir.join("data"));
        for i in 0..HISTORY_MAX + 5 {
            store.add_history(entry(&i.to_string())).unwrap();
        }
        let all = store.history();
        assert_eq!(all.len(), HISTORY_MAX);
        assert_eq!(all[0].id, (HISTORY_MAX + 4).to_string());
        store.clear_history().unwrap();
        assert!(store.history().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreadable_file_is_set_aside() {
        let dir = temp();
        let store = Store::new(&dir, &dir);
        fs::write(dir.join("apps.json"), "{ not json").unwrap();
        assert!(store.apps().is_empty());
        assert!(dir.join("apps.json.bad").exists());
        assert!(!dir.join("apps.json").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
