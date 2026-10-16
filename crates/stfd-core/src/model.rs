//! What STFD keeps: the apps, the settings and the history of what it did.

use crate::paths;
use crate::snapshot::Proc;
use serde::{Deserialize, Serialize};

/// When Windows starts a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Boot,
    System,
    Automatic,
    Manual,
    Disabled,
}

/// What STFD changed on a service, so removing the app can put it back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceOriginal {
    /// The start type before STFD set it to Manual; None when STFD left it alone.
    #[serde(default)]
    pub start_type: Option<StartType>,
    /// STFD let this person start and stop the service (removing the app takes that entry out
    /// again, and nothing else).
    #[serde(default)]
    pub granted: bool,
}

impl ServiceOriginal {
    pub fn is_empty(&self) -> bool {
        self.start_type.is_none() && !self.granted
    }
}

/// A service that comes and goes with an app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRule {
    /// The service's key name (`NIDomainService`), which never changes with the language.
    pub name: String,
    /// The name Windows shows (`NI Domain Service`).
    pub display: String,
    /// Start it before the program opens.
    pub start: bool,
    /// Stop it after the program closes.
    pub stop: bool,
    #[serde(default)]
    pub original: ServiceOriginal,
}

/// A shortcut of the program that STFD moved aside to put its own in place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcuts {
    /// Shortcut files STFD made for the app.
    #[serde(default)]
    pub made: Vec<String>,
    /// The program's own shortcuts STFD moved aside.
    #[serde(default)]
    pub moved: Vec<Moved>,
}

fn yes() -> bool {
    true
}

fn default_services_delay() -> u32 {
    3
}

/// An app STFD opens: a program, the folders whose programs and services belong to it, and
/// what to do around it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: String,
    pub name: String,
    /// The program the shortcut opens.
    pub program: String,
    #[serde(default)]
    pub arguments: String,
    /// Everything that runs from these folders belongs to the app.
    pub folders: Vec<String>,
    #[serde(default)]
    pub services: Vec<ServiceRule>,
    /// Programs outside the folders to close as well.
    #[serde(default)]
    pub also_close: Vec<String>,
    /// Programs inside the folders never to close (and not to count as the app being open).
    #[serde(default)]
    pub keep: Vec<String>,
    /// Seconds to wait after the program closes before closing the rest; None uses the default.
    #[serde(default)]
    pub wait_seconds: Option<u32>,
    /// Seconds to wait after starting services before opening the program, for services that say
    /// they are running before they answer.
    #[serde(default = "default_services_delay")]
    pub services_delay_seconds: u32,
    /// Close the programs left running from the folders.
    #[serde(default = "yes")]
    pub close_programs: bool,
    #[serde(default)]
    pub shortcuts: Shortcuts,
    /// Programs seen running with the app, most recent first, for choosing what else to close.
    #[serde(default)]
    pub seen: Vec<String>,
    pub created_at: u64,
}

/// How many programs an app remembers having seen.
pub const SEEN_MAX: usize = 60;

impl App {
    /// True for the program the shortcut opens.
    pub fn is_main(&self, path: &str) -> bool {
        paths::same(path, &self.program)
    }

    pub fn in_folders(&self, path: &str) -> bool {
        self.folders.iter().any(|f| paths::is_inside(path, f))
    }

    pub fn is_kept(&self, path: &str) -> bool {
        self.keep.iter().any(|k| paths::same(k, path))
    }

    /// True for a program that belongs to the app: the main program, anything in its folders, or a
    /// program it closes as well; never one it keeps.
    pub fn owns(&self, path: &str) -> bool {
        !self.is_kept(path)
            && (self.is_main(path) || self.in_folders(path) || self.also_close.iter().any(|a| paths::same(a, path)))
    }

    /// The app is open while its program runs, or while any of its programs shows a window (a
    /// launcher may start the real program and quit).
    pub fn is_open(&self, procs: &[Proc], own_pid: u32) -> bool {
        procs.iter().any(|p| {
            p.pid != own_pid
                && p.path
                    .as_deref()
                    .is_some_and(|path| self.is_main(path) || (p.windowed && self.owns(path)))
        })
    }

    pub fn wait_ms(&self, settings: &Settings) -> u64 {
        u64::from(self.wait_seconds.unwrap_or(settings.wait_seconds)) * 1000
    }

    /// Remembers programs seen with the app, most recent first, at most SEEN_MAX.
    pub fn remember(&mut self, paths_seen: &[String]) -> bool {
        let mut changed = false;
        for p in paths_seen.iter().rev() {
            if let Some(i) = self.seen.iter().position(|s| paths::same(s, p)) {
                if i == 0 {
                    continue;
                }
                let s = self.seen.remove(i);
                self.seen.insert(0, s);
            } else {
                self.seen.insert(0, p.clone());
            }
            changed = true;
        }
        self.seen.truncate(SEEN_MAX);
        changed
    }

    pub fn service(&self, name: &str) -> Option<&ServiceRule> {
        self.services.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// Settings for every app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Seconds to wait after a program closes before closing the rest.
    pub wait_seconds: u32,
    /// Seconds a program gets to close on its own before it is ended.
    pub polite_seconds: u32,
    /// A Windows notification after STFD closed something.
    pub notify: bool,
    /// Watch the apps even when they are opened another way (Start menu, a file, the taskbar); keeps
    /// STFD running in the tray and starting with Windows.
    pub watch_always: bool,
    pub language: String,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wait_seconds: 30,
            polite_seconds: 10,
            notify: true,
            watch_always: false,
            language: "system".into(),
            theme: "system".into(),
        }
    }
}

impl Settings {
    /// Keeps values in their range and falls back on unknown words.
    pub fn sanitized(mut self) -> Self {
        self.wait_seconds = self.wait_seconds.min(3600);
        self.polite_seconds = self.polite_seconds.clamp(1, 120);
        if !["system", "en", "tr"].contains(&self.language.as_str()) {
            self.language = "system".into();
        }
        if !["system", "dark", "light"].contains(&self.theme.as_str()) {
            self.theme = "system".into();
        }
        self
    }
}

/// What opened the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// STFD's shortcut (or Open in the window).
    Shortcut,
    /// Opened another way, and noticed because STFD watches always.
    Adopted,
    /// "Close leftovers now" in the window.
    Manual,
}

/// How a session ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// STFD closed what was left.
    Cleaned,
    /// Nothing was left to close.
    Nothing,
    /// The person chose to leave everything running this time.
    Kept,
    /// The program could not be opened.
    NotOpened,
}

/// Something that did not work, with a stable code the window translates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// The program or service it was about.
    pub what: String,
    /// `start_failed`, `stop_failed`, `close_failed`, `access_denied`, `launch_failed`,
    /// `dependent_running`, `in_use`, `service_process`, `other_user`.
    pub code: String,
    #[serde(default)]
    pub detail: String,
}

/// One time an app was opened and closed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub app_id: String,
    pub app_name: String,
    pub trigger: Trigger,
    pub outcome: Outcome,
    pub opened_at: Option<u64>,
    pub ended_at: u64,
    /// Services started before the program opened (display names).
    #[serde(default)]
    pub started: Vec<String>,
    /// Services stopped afterwards.
    #[serde(default)]
    pub stopped: Vec<String>,
    /// Programs that closed when asked.
    #[serde(default)]
    pub closed: Vec<String>,
    /// Programs that had to be ended.
    #[serde(default)]
    pub ended: Vec<String>,
    #[serde(default)]
    pub problems: Vec<Problem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn multisim() -> App {
        App {
            id: "ms".into(),
            name: "Multisim".into(),
            program: "C:\\Program Files (x86)\\National Instruments\\Circuit Design Suite 14.3\\multisim.exe".into(),
            arguments: String::new(),
            folders: vec!["C:\\Program Files (x86)\\National Instruments".into()],
            services: vec![],
            also_close: vec!["C:\\Tools\\helper.exe".into()],
            keep: vec!["C:\\Program Files (x86)\\National Instruments\\MAX\\NIMax.exe".into()],
            wait_seconds: None,
            services_delay_seconds: 3,
            close_programs: true,
            shortcuts: Shortcuts::default(),
            seen: vec![],
            created_at: 0,
        }
    }

    fn proc(pid: u32, path: &str, windowed: bool) -> Proc {
        Proc {
            pid,
            parent: 1,
            name: paths::file_name(path).to_string(),
            path: Some(path.to_string()),
            windowed,
            session: 1,
        }
    }

    #[test]
    fn what_belongs_to_an_app() {
        let app = multisim();
        assert!(app.owns("c:\\program files (x86)\\national instruments\\shared\\nisvc.exe"));
        assert!(app.owns("C:\\Tools\\helper.exe"));
        assert!(
            !app.owns("C:\\Program Files (x86)\\National Instruments\\MAX\\NIMax.exe"),
            "kept"
        );
        assert!(!app.owns("C:\\Program Files\\Other\\x.exe"));
    }

    #[test]
    fn open_while_the_program_runs_or_any_of_its_programs_shows_a_window() {
        let app = multisim();
        let main = proc(10, &app.program, false);
        let windowed_helper = proc(
            11,
            "C:\\Program Files (x86)\\National Instruments\\Shared\\ui.exe",
            true,
        );
        let background = proc(
            12,
            "C:\\Program Files (x86)\\National Instruments\\Shared\\svc.exe",
            false,
        );
        let kept_window = proc(
            13,
            "C:\\Program Files (x86)\\National Instruments\\MAX\\NIMax.exe",
            true,
        );
        let other = proc(14, "C:\\Program Files\\Other\\x.exe", true);

        assert!(app.is_open(std::slice::from_ref(&main), 1));
        assert!(app.is_open(&[windowed_helper], 1));
        assert!(!app.is_open(&[background, kept_window, other], 1));
        assert!(!app.is_open(&[main], 10), "STFD itself does not count");
    }

    #[test]
    fn the_wait_is_the_apps_own_or_the_default() {
        let mut app = multisim();
        let settings = Settings::default();
        assert_eq!(app.wait_ms(&settings), 30_000);
        app.wait_seconds = Some(0);
        assert_eq!(app.wait_ms(&settings), 0);
    }

    #[test]
    fn seen_programs_are_most_recent_first_without_duplicates() {
        let mut app = multisim();
        assert!(app.remember(&["C:\\a.exe".into(), "C:\\b.exe".into()]));
        assert_eq!(app.seen, vec!["C:\\a.exe", "C:\\b.exe"]);
        assert!(app.remember(&["c:\\B.EXE".into()]));
        assert_eq!(app.seen, vec!["C:\\b.exe", "C:\\a.exe"]);
        assert!(!app.remember(&["C:\\b.exe".into()]), "already first");
        let many: Vec<String> = (0..100).map(|i| format!("C:\\p{i}.exe")).collect();
        app.remember(&many);
        assert_eq!(app.seen.len(), SEEN_MAX);
        assert_eq!(app.seen[0], "C:\\p0.exe");
    }

    #[test]
    fn settings_stay_in_range() {
        let s = Settings {
            wait_seconds: 99_999,
            polite_seconds: 0,
            language: "xx".into(),
            theme: "pink".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!((s.wait_seconds, s.polite_seconds), (3600, 1));
        assert_eq!((s.language.as_str(), s.theme.as_str()), ("system", "system"));
    }

    #[test]
    fn an_app_saved_by_an_older_version_still_reads() {
        let json = r#"{"id":"a","name":"A","program":"C:\\A\\a.exe","folders":["C:\\A"],"createdAt":5}"#;
        let app: App = serde_json::from_str(json).unwrap();
        assert!(app.close_programs);
        assert_eq!(app.services_delay_seconds, 3);
        assert!(app.services.is_empty() && app.wait_seconds.is_none());
    }
}
