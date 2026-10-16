//! Setting apps up and taking them down: what the window shows while choosing (the programs in a
//! folder, the services that belong, the program's own shortcuts), saving with the one elevated
//! step when services need it, STFD's shortcuts, and undoing all of it when an app is removed.

use crate::commands::{CmdError, Res};
use crate::state::Shared;
use serde::{Deserialize, Serialize};
use std::path::Path;
use stfd_core::candidates::{self, ExeFile};
use stfd_core::elevated::{self, Change, Outcome, Request};
use stfd_core::model::{App, Moved, Problem, ServiceRule, StartType};
use stfd_core::paths::{self, FolderProblem};
use stfd_core::services::{ServiceInfo, Suggested};
use stfd_core::windows::{autostart, folders, icon, launch, services, shortcut, version};
use stfd_core::{new_id, now_ms};

/// Keeps the sign-in entry in step with the setting (and with where STFD is installed now).
pub fn sync_autostart(shared: &Shared) {
    if shared.settings().watch_always {
        let _ = autostart::set(&format!("\"{}\" --background", shared.own_exe));
    } else {
        autostart::remove();
    }
}

/// A program's icon as a data URL.
pub fn icon_url(path: &str) -> Option<String> {
    let png = icon::png(path, 48)?;
    Some(format!("data:image/png;base64,{}", base64(&png)))
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub folder: String,
    pub problem: Option<FolderProblem>,
    /// The programs in it, the likely main one first.
    pub programs: Vec<ExeFile>,
    /// The same vendor's folders elsewhere, worth watching too.
    pub vendor_folders: Vec<String>,
}

pub fn inspect_folder(shared: &Shared, folder: &str) -> FolderInfo {
    let folder = stfd_core::windows::long_path(folder.trim());
    let problem = paths::check_folder(&folder, &shared.known).err();
    if problem.is_some() || !Path::new(&folder).is_dir() {
        return FolderInfo {
            problem: problem.or(Some(FolderProblem::NotAbsolute)),
            folder,
            programs: Vec::new(),
            vendor_folders: Vec::new(),
        };
    }
    let mut programs = candidates::scan(&folder, 3000, 40, &|p| version::strings(p).name());
    programs.truncate(300);
    FolderInfo {
        vendor_folders: paths::vendor_folders(&folder, &shared.known, |p| Path::new(p).is_dir()),
        folder,
        problem: None,
        programs,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramInfo {
    pub path: String,
    pub name: String,
    pub company: Option<String>,
    pub description: Option<String>,
}

pub fn inspect_program(path: &str) -> Res<ProgramInfo> {
    let path = stfd_core::windows::long_path(path.trim());
    if !Path::new(&path).is_file() || !path.to_lowercase().ends_with(".exe") {
        return Err(CmdError::new("invalid", "not a program"));
    }
    let v = version::strings(&path);
    let stem = paths::file_name(&path);
    let stem = stem.get(..stem.len() - 4).unwrap_or(stem).to_string();
    Ok(ProgramInfo {
        name: v.name().unwrap_or(stem),
        company: v.company,
        description: v.description,
        path,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceChoice {
    pub company: Option<String>,
    /// The services that look like the app's.
    pub related: Vec<Suggested>,
    /// Every service, for adding one by hand.
    pub all: Vec<ServiceInfo>,
}

pub fn list_services(shared: &Shared, folders: &[String], program: &str) -> Res<ServiceChoice> {
    let mut all = services::list(&shared.known).map_err(|e| CmdError::new("services_failed", e))?;
    let company = version::strings(program).company;
    let related = stfd_core::services::related(&all, folders, company.as_deref(), &shared.known);
    all.sort_by_key(|s| s.display.to_lowercase());
    Ok(ServiceChoice { company, related, all })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundShortcut {
    pub path: String,
    /// On the person's own desktop: STFD may move it aside. The shared desktop needs an
    /// administrator.
    pub movable: bool,
}

pub fn find_shortcuts(program: &str) -> Vec<FoundShortcut> {
    let mut out = Vec::new();
    if let Some(d) = folders::desktop() {
        out.extend(
            shortcut::find_for(program, &[d])
                .into_iter()
                .map(|path| FoundShortcut { path, movable: true }),
        );
    }
    if let Some(d) = folders::public_desktop() {
        out.extend(
            shortcut::find_for(program, &[d])
                .into_iter()
                .map(|path| FoundShortcut { path, movable: false }),
        );
    }
    out
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftService {
    pub name: String,
    pub display: String,
    pub start: bool,
    pub stop: bool,
}

/// An app as the window sends it when saving.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub id: Option<String>,
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub arguments: String,
    pub folders: Vec<String>,
    #[serde(default)]
    pub services: Vec<DraftService>,
    /// Services to set to start by hand.
    #[serde(default)]
    pub manual: Vec<String>,
    #[serde(default)]
    pub also_close: Vec<String>,
    #[serde(default)]
    pub keep: Vec<String>,
    pub wait_seconds: Option<u32>,
    #[serde(default)]
    pub services_delay_seconds: u32,
    #[serde(default = "yes")]
    pub close_programs: bool,
    #[serde(default)]
    pub desktop: bool,
    #[serde(default)]
    pub start_menu: bool,
    /// The program's own shortcuts to move aside for STFD's.
    #[serde(default)]
    pub replace: Vec<String>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// Nothing needed administrator rights.
    NotNeeded,
    Granted,
    /// The person said no to Windows' question.
    Declined,
    Failed,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    pub app: App,
    pub permission: Permission,
    pub problems: Vec<Problem>,
}

/// Checks a draft and turns it into an app, keeping what an existing one learned.
fn build(shared: &Shared, draft: &Draft) -> Res<App> {
    let name = draft.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(CmdError::new("invalid", "name"));
    }
    let program = stfd_core::windows::long_path(draft.program.trim());
    if !Path::new(&program).is_file() || !program.to_lowercase().ends_with(".exe") {
        return Err(CmdError::new("invalid", "program"));
    }
    if draft.folders.is_empty() {
        return Err(CmdError::new("invalid", "folders"));
    }
    let mut folders_ok = Vec::new();
    for f in &draft.folders {
        let f = stfd_core::windows::long_path(f.trim());
        if let Err(problem) = paths::check_folder(&f, &shared.known) {
            return Err(CmdError::new("folder_problem", format!("{f}: {problem:?}")));
        }
        if !folders_ok.iter().any(|o: &String| paths::same(o, &f)) {
            folders_ok.push(f);
        }
    }
    let existing = draft.id.as_deref().and_then(|id| shared.app(id));
    let rules = draft
        .services
        .iter()
        .filter(|s| !s.name.trim().is_empty())
        .map(|s| ServiceRule {
            name: s.name.clone(),
            display: if s.display.trim().is_empty() {
                s.name.clone()
            } else {
                s.display.clone()
            },
            start: s.start,
            stop: s.stop,
            original: existing
                .as_ref()
                .and_then(|e| e.service(&s.name))
                .map(|r| r.original.clone())
                .unwrap_or_default(),
        })
        .collect();
    Ok(App {
        id: existing.as_ref().map(|e| e.id.clone()).unwrap_or_else(new_id),
        name: name.to_string(),
        program,
        arguments: draft.arguments.trim().to_string(),
        folders: folders_ok,
        services: rules,
        also_close: draft.also_close.clone(),
        keep: draft.keep.clone(),
        wait_seconds: draft.wait_seconds.map(|w| w.min(3600)),
        services_delay_seconds: draft.services_delay_seconds.min(120),
        close_programs: draft.close_programs,
        shortcuts: existing.as_ref().map(|e| e.shortcuts.clone()).unwrap_or_default(),
        seen: existing.as_ref().map(|e| e.seen.clone()).unwrap_or_default(),
        created_at: existing.as_ref().map(|e| e.created_at).unwrap_or_else(now_ms),
    })
}

/// Runs the elevated step for `changes`; the outcomes, or how it did not happen.
fn elevate(shared: &Shared, changes: Vec<Change>) -> Result<Vec<Outcome>, Permission> {
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    let sid = services::own_sid().map_err(|_| Permission::Failed)?;
    let request = Request { sid, services: changes };
    let dir = shared.store.data_dir().to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let result = dir.join(format!("elevated-{}.json", new_id()));
    let args = format!("--elevated {} \"{}\"", elevated::encode(&request), result.display());
    let code = launch::run_elevated(&shared.own_exe, &args).map_err(|_| Permission::Failed)?;
    let outcomes: Vec<Outcome> = stfd_core::store::read(&result);
    let _ = std::fs::remove_file(&result);
    match code {
        None => Err(Permission::Declined),
        Some(0) => Ok(outcomes),
        Some(_) => Err(Permission::Failed),
    }
}

/// The permission and start-type changes an app's services need.
fn needed_changes(app: &App, manual: &[String]) -> Vec<Change> {
    app.services
        .iter()
        .filter_map(|r| {
            let grant = (r.start || r.stop) && !services::controllable(&r.name);
            let make_manual = manual.iter().any(|m| m.eq_ignore_ascii_case(&r.name))
                && services::get_start_type(&r.name).is_ok_and(|t| t == StartType::Automatic);
            (grant || make_manual).then(|| Change {
                name: r.name.clone(),
                grant,
                manual: make_manual,
                ..Change::default()
            })
        })
        .collect()
}

fn apply_outcomes(app: &mut App, outcomes: &[Outcome]) -> Vec<Problem> {
    let mut problems = Vec::new();
    for o in outcomes {
        if let Some(rule) = app.services.iter_mut().find(|r| r.name.eq_ignore_ascii_case(&o.name)) {
            rule.original.granted |= o.original.granted;
            if rule.original.start_type.is_none() {
                rule.original.start_type = o.original.start_type;
            }
        }
        if let Some(e) = &o.error {
            problems.push(Problem {
                what: o.name.clone(),
                code: "permission_failed".into(),
                detail: e.clone(),
            });
        }
    }
    problems
}

/// Gives the person the rights an app's services need (one Windows question), and remembers what
/// changed.
pub fn grant(shared: &Shared, app: &mut App, manual: &[String]) -> (Permission, Vec<Problem>) {
    let changes = needed_changes(app, manual);
    if changes.is_empty() {
        return (Permission::NotNeeded, Vec::new());
    }
    match elevate(shared, changes) {
        Ok(outcomes) => {
            let problems = apply_outcomes(app, &outcomes);
            crate::watcher::permissions_changed(shared);
            (
                if problems.is_empty() {
                    Permission::Granted
                } else {
                    Permission::Failed
                },
                problems,
            )
        }
        Err(p) => (p, Vec::new()),
    }
}

fn store_app(shared: &Shared, app: &App) -> Res<()> {
    let mut apps = crate::state::lock(&shared.apps);
    match apps.iter_mut().find(|a| a.id == app.id) {
        Some(existing) => *existing = app.clone(),
        None => apps.push(app.clone()),
    }
    shared
        .store
        .save_apps(&apps)
        .map_err(|e| CmdError::new("save_failed", e))
}

pub fn save(shared: &Shared, draft: &Draft) -> Res<SaveOutcome> {
    let mut app = build(shared, draft)?;
    let (permission, mut problems) = grant(shared, &mut app, &draft.manual);
    if draft.desktop || draft.start_menu {
        problems.extend(make_shortcuts(
            shared,
            &mut app,
            draft.desktop,
            draft.start_menu,
            &draft.replace,
        ));
    }
    store_app(shared, &app)?;
    Ok(SaveOutcome {
        app,
        permission,
        problems,
    })
}

/// Saves an edited app as it is, without the elevated step (the window offers it when a service
/// needs it).
pub fn update(shared: &Shared, app: App) -> Res<App> {
    let current = shared.app(&app.id).ok_or_else(|| CmdError::new("not_found", &app.id))?;
    let draft = Draft {
        id: Some(app.id.clone()),
        name: app.name.clone(),
        program: app.program.clone(),
        arguments: app.arguments.clone(),
        folders: app.folders.clone(),
        services: app
            .services
            .iter()
            .map(|r| DraftService {
                name: r.name.clone(),
                display: r.display.clone(),
                start: r.start,
                stop: r.stop,
            })
            .collect(),
        manual: Vec::new(),
        also_close: app.also_close.clone(),
        keep: app.keep.clone(),
        wait_seconds: app.wait_seconds,
        services_delay_seconds: app.services_delay_seconds,
        close_programs: app.close_programs,
        desktop: false,
        start_menu: false,
        replace: Vec::new(),
    };
    let mut built = build(shared, &draft)?;
    built.shortcuts = current.shortcuts;
    built.seen = current.seen;
    store_app(shared, &built)?;
    Ok(built)
}

pub fn grant_for(shared: &Shared, id: &str) -> Res<SaveOutcome> {
    let mut app = shared.app(id).ok_or_else(|| CmdError::new("not_found", id))?;
    let (permission, problems) = grant(shared, &mut app, &[]);
    store_app(shared, &app)?;
    Ok(SaveOutcome {
        app,
        permission,
        problems,
    })
}

fn is_ours(shared: &Shared, link: &str, id: &str) -> bool {
    shortcut::read(link)
        .is_ok_and(|(target, args)| paths::same(&target, &shared.own_exe) && args.trim() == format!("--run {id}"))
}

/// Makes STFD's shortcuts for the app (again): on the desktop and/or in the Start menu, after
/// moving the program's own desktop shortcuts in `replace` aside.
pub fn make_shortcuts(
    shared: &Shared,
    app: &mut App,
    desktop: bool,
    start_menu: bool,
    replace: &[String],
) -> Vec<Problem> {
    let mut problems = Vec::new();
    // Earlier ones go first (the name may have changed).
    for made in std::mem::take(&mut app.shortcuts.made) {
        if is_ours(shared, &made, &app.id) {
            let _ = std::fs::remove_file(&made);
        }
    }
    let user_desktop = folders::desktop();
    let backup = shared.store.data_dir().join("shortcuts").join(&app.id);
    for r in replace {
        let own_desktop = user_desktop
            .as_deref()
            .is_some_and(|d| paths::parent(r).is_some_and(|p| paths::same(p, d)));
        if !own_desktop || !Path::new(r).is_file() {
            continue;
        }
        let _ = std::fs::create_dir_all(&backup);
        let to = backup.join(paths::file_name(r));
        match std::fs::rename(r, &to) {
            Ok(()) => app.shortcuts.moved.push(Moved {
                from: r.clone(),
                to: to.to_string_lossy().to_string(),
            }),
            Err(e) => problems.push(Problem {
                what: paths::file_name(r).to_string(),
                code: "shortcut_failed".into(),
                detail: e.to_string(),
            }),
        }
    }

    let mut targets = Vec::new();
    if desktop {
        targets.extend(user_desktop);
    }
    if start_menu {
        targets.extend(folders::start_menu());
    }
    let description = format!("Opens {} with STFD", app.name);
    let arguments = format!("--run {}", app.id);
    let working_dir = paths::parent(&app.program).unwrap_or("").to_string();
    for folder in targets {
        let mut path = format!(
            "{}\\{}",
            folder.trim_end_matches('\\'),
            paths::shortcut_file_name(&app.name)
        );
        if Path::new(&path).exists() && !is_ours(shared, &path, &app.id) {
            path = format!(
                "{}\\{}",
                folder.trim_end_matches('\\'),
                paths::shortcut_file_name(&format!("{} (STFD)", app.name))
            );
        }
        let link = shortcut::Link {
            target: &shared.own_exe,
            arguments: &arguments,
            working_dir: &working_dir,
            icon: &app.program,
            description: &description,
        };
        match shortcut::create(&path, &link) {
            Ok(()) => app.shortcuts.made.push(path),
            Err(e) => problems.push(Problem {
                what: paths::file_name(&path).to_string(),
                code: "shortcut_failed".into(),
                detail: e.to_string(),
            }),
        }
    }
    problems
}

pub fn make_shortcuts_for(shared: &Shared, id: &str, desktop: bool, start_menu: bool) -> Res<App> {
    let mut app = shared.app(id).ok_or_else(|| CmdError::new("not_found", id))?;
    let problems = make_shortcuts(shared, &mut app, desktop, start_menu, &[]);
    store_app(shared, &app)?;
    if let Some(p) = problems.first() {
        return Err(CmdError::new("shortcut_failed", &p.detail));
    }
    Ok(app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveOutcome {
    pub permission: Permission,
    pub problems: Vec<Problem>,
}

/// Removes an app: its shortcuts go, the program's own come back, and what STFD changed on its
/// services is put back unless another app still uses them.
pub fn remove(shared: &Shared, id: &str) -> Res<RemoveOutcome> {
    let app = shared.app(id).ok_or_else(|| CmdError::new("not_found", id))?;
    crate::state::lock(&shared.sessions).remove(id);
    let mut problems = Vec::new();
    for made in &app.shortcuts.made {
        if is_ours(shared, made, id) {
            let _ = std::fs::remove_file(made);
        }
    }
    for m in &app.shortcuts.moved {
        if Path::new(&m.from).exists() || std::fs::rename(&m.to, &m.from).is_err() {
            problems.push(Problem {
                what: paths::file_name(&m.from).to_string(),
                code: "shortcut_not_restored".into(),
                detail: m.to.clone(),
            });
        }
    }

    let others: Vec<App> = shared.apps().into_iter().filter(|a| a.id != id).collect();
    let changes: Vec<Change> = app
        .services
        .iter()
        .filter(|r| !r.original.is_empty())
        .filter(|r| !others.iter().any(|o| o.service(&r.name).is_some()))
        .map(|r| Change {
            name: r.name.clone(),
            revoke: r.original.granted,
            restore_start: r.original.start_type,
            ..Change::default()
        })
        .collect();
    let permission = if changes.is_empty() {
        Permission::NotNeeded
    } else {
        match elevate(shared, changes) {
            Ok(outcomes) => {
                for o in outcomes.iter().filter(|o| o.error.is_some()) {
                    problems.push(Problem {
                        what: o.name.clone(),
                        code: "permission_failed".into(),
                        detail: o.error.clone().unwrap_or_default(),
                    });
                }
                Permission::Granted
            }
            Err(p) => p,
        }
    };

    let mut apps = crate::state::lock(&shared.apps);
    apps.retain(|a| a.id != id);
    shared
        .store
        .save_apps(&apps)
        .map_err(|e| CmdError::new("save_failed", e))?;
    Ok(RemoveOutcome { permission, problems })
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
