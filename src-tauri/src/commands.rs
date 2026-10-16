//! The commands the window calls. Errors carry a stable code the window translates.

use crate::state::{lock, AppStatus, Shared};
use serde::Serialize;
use std::sync::Arc;
use stfd_core::model::{App, HistoryEntry, Settings};
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
pub struct CmdError {
    code: &'static str,
    detail: String,
}

impl CmdError {
    pub fn new(code: &'static str, detail: impl ToString) -> Self {
        Self {
            code,
            detail: detail.to_string(),
        }
    }
}

pub type Res<T> = Result<T, CmdError>;

#[cfg(not(windows))]
fn unsupported<T>() -> Res<T> {
    Err(CmdError::new("unsupported", "STFD runs on Windows"))
}

/// Runs blocking work (reading every service, scanning a folder, the elevated step) off the
/// window's thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| Err(CmdError::new("internal", e)))
}

/// What the window shows, as of the watcher's last step.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// False on systems other than Windows, where STFD does nothing.
    pub supported: bool,
    pub now: u64,
    pub apps: Vec<App>,
    pub status: Vec<AppStatus>,
}

#[tauri::command]
pub fn get_state(shared: State<'_, Arc<Shared>>) -> Snapshot {
    Snapshot {
        supported: cfg!(windows),
        now: stfd_core::now_ms(),
        apps: shared.apps(),
        status: lock(&shared.status).clone(),
    }
}

/// A program's icon as a data URL (kept once read).
#[tauri::command]
pub async fn get_icon(shared: State<'_, Arc<Shared>>, path: String) -> Res<Option<String>> {
    if let Some(known) = lock(&shared.icons).get(&path) {
        return Ok(known.clone());
    }
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        let url = crate::setup::icon_url(&path);
        #[cfg(not(windows))]
        let url = None;
        lock(&shared.icons).insert(path, url.clone());
        Ok(url)
    })
    .await
}

#[tauri::command]
pub fn get_settings(shared: State<'_, Arc<Shared>>) -> Settings {
    shared.settings()
}

#[tauri::command]
pub fn set_settings(shared: State<'_, Arc<Shared>>, settings: Settings) -> Res<Settings> {
    let settings = settings.sanitized();
    shared
        .store
        .save_settings(&settings)
        .map_err(|e| CmdError::new("settings_not_saved", e))?;
    *lock(&shared.settings) = settings.clone();
    #[cfg(windows)]
    crate::setup::sync_autostart(&shared);
    Ok(settings)
}

#[tauri::command]
pub fn get_history(shared: State<'_, Arc<Shared>>) -> Vec<HistoryEntry> {
    shared.store.history()
}

#[tauri::command]
pub fn clear_history(shared: State<'_, Arc<Shared>>) -> Res<()> {
    shared
        .store
        .clear_history()
        .map_err(|e| CmdError::new("history_failed", e))
}

#[tauri::command]
pub async fn inspect_folder(shared: State<'_, Arc<Shared>>, folder: String) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::inspect_folder(&shared, &folder))
                .map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, folder);
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn inspect_program(path: String) -> Res<serde_json::Value> {
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::inspect_program(&path)?).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn list_services(
    shared: State<'_, Arc<Shared>>,
    folders: Vec<String>,
    program: String,
) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::list_services(&shared, &folders, &program)?)
                .map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, folders, program);
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn find_shortcuts(program: String) -> Res<serde_json::Value> {
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::find_shortcuts(&program)).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = program;
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn save_app(shared: State<'_, Arc<Shared>>, draft: serde_json::Value) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            let draft: crate::setup::Draft = serde_json::from_value(draft).map_err(|e| CmdError::new("invalid", e))?;
            serde_json::to_value(crate::setup::save(&shared, &draft)?).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, draft);
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn update_app(shared: State<'_, Arc<Shared>>, app: App) -> Res<App> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            crate::setup::update(&shared, app)
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, app);
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn grant_permission(shared: State<'_, Arc<Shared>>, id: String) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::grant_for(&shared, &id)?).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, id);
            unsupported()
        }
    })
    .await
}

#[tauri::command]
pub async fn remove_app(shared: State<'_, Arc<Shared>>, id: String) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            serde_json::to_value(crate::setup::remove(&shared, &id)?).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, id);
            unsupported()
        }
    })
    .await
}

/// Opens the app the way its shortcut does.
#[tauri::command]
pub fn open_app(app: AppHandle, shared: State<'_, Arc<Shared>>, id: String) -> Res<()> {
    #[cfg(windows)]
    {
        if shared.app(&id).is_none() {
            return Err(CmdError::new("not_found", id));
        }
        crate::watcher::launch(&app, &shared, &id);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, shared, id);
        unsupported()
    }
}

/// What "Close leftovers now" would close.
#[tauri::command]
pub async fn preview_cleanup(shared: State<'_, Arc<Shared>>, id: String) -> Res<serde_json::Value> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            let plan = crate::watcher::preview(&shared, &id).ok_or_else(|| CmdError::new("not_found", &id))?;
            serde_json::to_value(plan).map_err(|e| CmdError::new("internal", e))
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, id);
            unsupported()
        }
    })
    .await
}

/// Closes what the app left now; None when it was waiting and the watcher does it on its next step.
#[tauri::command]
pub async fn close_now(app: AppHandle, shared: State<'_, Arc<Shared>>, id: String) -> Res<Option<HistoryEntry>> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            Ok(crate::watcher::close_now(&app, &shared, &id))
        }
        #[cfg(not(windows))]
        {
            let _ = (app, shared, id);
            unsupported()
        }
    })
    .await
}

/// Leaves everything running this time.
#[tauri::command]
pub fn keep_running(shared: State<'_, Arc<Shared>>, id: String) {
    #[cfg(windows)]
    crate::watcher::keep_running(&shared, &id);
    #[cfg(not(windows))]
    let _ = (shared, id);
}

#[tauri::command]
pub async fn make_shortcuts(shared: State<'_, Arc<Shared>>, id: String, desktop: bool, start_menu: bool) -> Res<App> {
    let shared = Arc::clone(&shared);
    blocking(move || {
        #[cfg(windows)]
        {
            crate::setup::make_shortcuts_for(&shared, &id, desktop, start_menu)
        }
        #[cfg(not(windows))]
        {
            let _ = (shared, id, desktop, start_menu);
            unsupported()
        }
    })
    .await
}
