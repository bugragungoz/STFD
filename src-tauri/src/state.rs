//! What the app holds while it runs: the apps and settings (kept on disk by the core's Store), the
//! sessions being followed, and the last status the watcher computed for the window.

use lockstep_core::model::{App, Problem, Settings};
use lockstep_core::paths::KnownFolders;
use lockstep_core::plan::Target;
use lockstep_core::session::{Phase, Session};
use lockstep_core::snapshot::SvcState;
use lockstep_core::store::Store;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

/// A session and what went wrong on the way (shown in History when it ends).
#[derive(Clone, Debug)]
pub struct Live {
    pub session: Session,
    pub problems: Vec<Problem>,
    /// The program could not be opened.
    pub not_opened: bool,
    /// Services are starting and the program is about to open: the watcher leaves the session
    /// alone, and another launch of the same app waits for this one.
    pub launching: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub name: String,
    /// None when this Windows has no such service.
    pub state: Option<SvcState>,
    /// The person may start and stop it without administrator rights.
    pub controllable: bool,
}

/// One app as the window shows it.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub id: String,
    /// None while the app is not open and Lockstep is not following it.
    pub phase: Option<Phase>,
    pub opened_at: Option<u64>,
    /// Its programs running now.
    pub running: Vec<Target>,
    pub services: Vec<ServiceStatus>,
}

/// A poisoned lock only means a panic elsewhere; the data is still the last good state.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Shared {
    pub store: Store,
    pub apps: Mutex<Vec<App>>,
    pub settings: Mutex<Settings>,
    pub sessions: Mutex<BTreeMap<String, Live>>,
    pub status: Mutex<Vec<AppStatus>>,
    /// Program icons as data URLs, by path.
    pub icons: Mutex<HashMap<String, Option<String>>>,
    /// Whether the person may start and stop a service (lowercase name), and when that was read.
    pub controllable: Mutex<HashMap<String, (bool, u64)>>,
    pub known: KnownFolders,
    /// Lockstep's own program, which its shortcuts open.
    pub own_exe: String,
    pub window_open: AtomicBool,
    /// Launches still starting services (no session decision yet).
    pub launching: AtomicUsize,
}

impl Shared {
    pub fn new(store: Store, known: KnownFolders, own_exe: String) -> Self {
        let apps = store.apps();
        let settings = store.settings();
        Self {
            store,
            apps: Mutex::new(apps),
            settings: Mutex::new(settings),
            sessions: Mutex::new(BTreeMap::new()),
            status: Mutex::new(Vec::new()),
            icons: Mutex::new(HashMap::new()),
            controllable: Mutex::new(HashMap::new()),
            known,
            own_exe,
            window_open: AtomicBool::new(false),
            launching: AtomicUsize::new(0),
        }
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn apps(&self) -> Vec<App> {
        lock(&self.apps).clone()
    }

    pub fn app(&self, id: &str) -> Option<App> {
        lock(&self.apps).iter().find(|a| a.id == id).cloned()
    }

    /// Changes one app and saves all of them.
    pub fn update_app(&self, id: &str, f: impl FnOnce(&mut App)) -> std::io::Result<Option<App>> {
        let mut apps = lock(&self.apps);
        let Some(app) = apps.iter_mut().find(|a| a.id == id) else {
            return Ok(None);
        };
        f(app);
        let out = app.clone();
        self.store.save_apps(&apps)?;
        Ok(Some(out))
    }

    /// True while Lockstep has something to do without its window: an app being followed, a launch
    /// on its way, or watching apps opened another way.
    pub fn busy(&self) -> bool {
        !lock(&self.sessions).is_empty()
            || self.launching.load(Ordering::SeqCst) > 0
            || lock(&self.settings).watch_always
    }
}
