//! The watcher: once a second it reads the processes and services, steps every session (open,
//! closing after the wait, cleaning up), notices apps opened another way when asked to, and keeps
//! the status the window shows. Opening an app (the shortcut) and cleaning up run on their own
//! threads so a slow service never holds the step.

use crate::state::{lock, AppStatus, Live, ServiceStatus, Shared};
use crate::{lang, ui};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use stfd_core::model::{App, HistoryEntry, Outcome, Problem, Trigger};
use stfd_core::plan::{self, Context, Skip};
use stfd_core::session::{Change, Phase, Session};
use stfd_core::snapshot::Snapshot;
use stfd_core::windows::launch::Opened;
use stfd_core::windows::{execute, launch, process, services};
use stfd_core::{new_id, now_ms};
use tauri::AppHandle;

const STEP: Duration = Duration::from_millis(1000);

pub fn start(app: AppHandle, shared: Arc<Shared>) {
    std::thread::spawn(move || {
        let mut cache = process::PathCache::default();
        let mut followed = usize::MAX;
        loop {
            step(&app, &shared, &mut cache);
            let now_followed = lock(&shared.sessions).len();
            if now_followed != followed {
                trace!("following {now_followed} apps");
                followed = now_followed;
            }
            if !shared.busy() && !shared.window_open.load(Ordering::SeqCst) {
                trace!("nothing left to follow: exiting");
                app.exit(0);
                // The main thread ends the app; should it not get to it, end the process here.
                std::thread::sleep(Duration::from_secs(5));
                trace!("the main thread did not exit: ending the process");
                std::process::exit(0);
            }
            std::thread::sleep(STEP);
        }
    });
}

fn others_open(shared: &Shared, apps: &[App], snap: &Snapshot, except: &str) -> Vec<App> {
    let sessions = lock(&shared.sessions);
    apps.iter()
        .filter(|a| a.id != except)
        .filter(|a| {
            let followed = sessions
                .get(&a.id)
                .is_some_and(|l| matches!(l.session.phase, Phase::Starting { .. } | Phase::Open { .. }));
            followed || a.is_open(&snap.procs, process::own_pid())
        })
        .cloned()
        .collect()
}

fn step(handle: &AppHandle, shared: &Arc<Shared>, cache: &mut process::PathCache) {
    let now = now_ms();
    let own = process::own_pid();
    let procs = match cache.processes() {
        Ok(p) => p,
        Err(e) => {
            trace!("reading the processes failed: {e}");
            return;
        }
    };
    let snap = Snapshot {
        procs,
        services: services::states().unwrap_or_default(),
    };
    let apps = shared.apps();
    let settings = shared.settings();

    // Apps opened another way: followed from now on, their services started late.
    if settings.watch_always {
        for a in &apps {
            let followed = lock(&shared.sessions).contains_key(&a.id);
            if followed || !a.is_open(&snap.procs, own) {
                continue;
            }
            lock(&shared.sessions).insert(
                a.id.clone(),
                Live {
                    session: Session::adopted(&a.id, now),
                    problems: Vec::new(),
                    not_opened: false,
                    launching: false,
                },
            );
            let names = plan::to_start(a, &snap);
            if !names.is_empty() {
                let (shared, known, id) = (shared.clone(), snap.services.clone(), a.id.clone());
                std::thread::spawn(move || {
                    let started = execute::start_services(&names, &known);
                    if let Some(live) = lock(&shared.sessions).get_mut(&id) {
                        live.session.started.extend(started.started);
                        live.problems.extend(started.problems);
                    }
                });
            }
        }
    }

    // Step each session.
    let mut cleanups = Vec::new();
    {
        let mut sessions = lock(&shared.sessions);
        sessions.retain(|id, _| apps.iter().any(|a| &a.id == id));
        for (id, live) in sessions.iter_mut() {
            let Some(app) = apps.iter().find(|a| &a.id == id) else {
                continue;
            };
            if live.session.is_cleaning() || live.launching {
                continue;
            }
            let open = app.is_open(&snap.procs, own);
            live.session.note_seen(plan::seen(app, &snap, &shared.known, own));
            let change = live.session.step(open, now, app.wait_ms(&settings));
            if change != Change::None {
                trace!("{id}: {change:?}");
            }
            if change == Change::Cleanup {
                cleanups.push(id.clone());
            }
        }
    }
    for id in cleanups {
        let (handle, shared) = (handle.clone(), shared.clone());
        std::thread::spawn(move || {
            cleanup(&handle, &shared, &id);
        });
    }

    // What the window shows; whether the person may start and stop each service is read again
    // every 15 seconds while the window is open.
    let window = shared.window_open.load(Ordering::SeqCst);
    let sessions = lock(&shared.sessions).clone();
    let mut controllable = lock(&shared.controllable);
    let status: Vec<AppStatus> = apps
        .iter()
        .map(|a| AppStatus {
            id: a.id.clone(),
            phase: sessions.get(&a.id).map(|l| l.session.phase),
            opened_at: sessions.get(&a.id).and_then(|l| l.session.opened_at),
            running: plan::running(a, &snap, own),
            services: a
                .services
                .iter()
                .map(|r| {
                    let entry = controllable.entry(r.name.to_lowercase()).or_insert((false, 0));
                    if window && now.saturating_sub(entry.1) > 15_000 {
                        *entry = (services::controllable(&r.name), now);
                    }
                    ServiceStatus {
                        name: r.name.clone(),
                        state: snap.service(&r.name).map(|s| s.state),
                        controllable: entry.0,
                    }
                })
                .collect(),
        })
        .collect();
    drop(controllable);
    *lock(&shared.status) = status;
    ui::update_tray(handle, shared, now);
}

/// Forgets what the person may do with services, after the elevated step changed it.
pub fn permissions_changed(shared: &Shared) {
    lock(&shared.controllable).clear();
}

fn problem_of(skipped: &plan::Skipped) -> Option<Problem> {
    let (code, detail) = match &skipped.reason {
        Skip::System => return None,
        Skip::OtherUser => ("other_user", String::new()),
        Skip::ServiceProcess { service } => ("service_process", service.clone()),
        Skip::InUse { app } => ("in_use", app.clone()),
        Skip::DependentRunning { service } => ("dependent_running", service.clone()),
    };
    Some(Problem {
        what: skipped.what.clone(),
        code: code.into(),
        detail,
    })
}

/// What closing an app would do now (for "Close leftovers now" to show first).
pub fn preview(shared: &Shared, id: &str) -> Option<plan::Plan> {
    let app = shared.app(id)?;
    let snap = Snapshot {
        procs: process::PathCache::default().processes().ok()?,
        services: services::states_with_dependencies().ok()?,
    };
    let apps = shared.apps();
    let others = others_open(shared, &apps, &snap, id);
    let own = process::own_pid();
    let ctx = Context {
        own_pid: own,
        own_session: process::session_of(own),
        known: &shared.known,
        others_open: &others,
    };
    Some(plan::cleanup(&app, &snap, &ctx))
}

/// Closes what the app left, writes it to History and ends the session.
pub fn cleanup(handle: &AppHandle, shared: &Arc<Shared>, id: &str) -> Option<HistoryEntry> {
    let app = shared.app(id)?;
    let live = lock(&shared.sessions).get(id).cloned()?;
    let settings = shared.settings();
    let own = process::own_pid();
    let own_session = process::session_of(own);

    let snap = Snapshot {
        procs: process::PathCache::default().processes().unwrap_or_default(),
        services: services::states_with_dependencies().unwrap_or_default(),
    };
    let apps = shared.apps();
    let others = others_open(shared, &apps, &snap, id);
    let ctx = Context {
        own_pid: own,
        own_session,
        known: &shared.known,
        others_open: &others,
    };
    let plan = plan::cleanup(&app, &snap, &ctx);
    trace!(
        "{id}: closing {} programs and {} services",
        plan.programs.len(),
        plan.services.len()
    );
    let done = execute::close(
        &plan,
        &snap.services,
        Duration::from_secs(settings.polite_seconds.into()),
        || {
            let again = Snapshot {
                procs: process::PathCache::default().processes().unwrap_or_default(),
                services: services::states().unwrap_or_default(),
            };
            plan::cleanup(&app, &again, &ctx).programs
        },
    );

    let mut problems = live.problems.clone();
    problems.extend(done.problems.iter().cloned());
    problems.extend(plan.skipped.iter().filter_map(problem_of));
    let outcome = if live.not_opened {
        Outcome::NotOpened
    } else if plan.is_empty() {
        Outcome::Nothing
    } else {
        Outcome::Cleaned
    };
    let entry = HistoryEntry {
        id: new_id(),
        app_id: app.id.clone(),
        app_name: app.name.clone(),
        trigger: live.session.trigger,
        outcome,
        opened_at: live.session.opened_at,
        ended_at: now_ms(),
        started: live.session.started.clone(),
        stopped: done.stopped.clone(),
        closed: done.closed.clone(),
        ended: done.ended.clone(),
        problems: problems.clone(),
    };
    let _ = shared.store.add_history(entry.clone());
    let _ = shared.update_app(id, |a| {
        a.remember(&live.session.seen);
    });
    lock(&shared.sessions).remove(id);
    trace!(
        "{id}: {outcome:?}, closed {:?}, ended {:?}, stopped {:?}",
        done.closed,
        done.ended,
        done.stopped
    );

    let words = lang::pick(&settings.language);
    if outcome == Outcome::NotOpened {
        let (title, body) = words.not_opened(&app.name);
        ui::notify(handle, shared, &title, &body);
    } else if live.session.trigger != Trigger::Manual && (outcome == Outcome::Cleaned || !problems.is_empty()) {
        let programs = done.closed.len() + done.ended.len();
        let (title, body) = words.cleaned(&app.name, done.stopped.len(), programs, problems.len());
        ui::notify(handle, shared, &title, &body);
    }
    Some(entry)
}

/// Opens an app the way its shortcut does: its services first, then the program; the watcher
/// follows it from there.
pub fn launch(handle: &AppHandle, shared: &Arc<Shared>, id: &str) {
    let Some(app) = shared.app(id) else {
        ui::show_window(handle);
        return;
    };
    shared.launching.fetch_add(1, Ordering::SeqCst);
    let (handle, shared) = (handle.clone(), shared.clone());
    std::thread::spawn(move || {
        launch_now(&handle, &shared, &app);
        shared.launching.fetch_sub(1, Ordering::SeqCst);
    });
}

fn open_program(shared: &Shared, app: &App) {
    let failed = match launch::open(&app.program, &app.arguments) {
        Ok(Opened::Pid(_)) => None,
        Ok(Opened::Cancelled) => Some("cancelled".to_string()),
        Err(e) => Some(e.to_string()),
    };
    trace!(
        "{}: opening the program: {}",
        app.id,
        failed.as_deref().unwrap_or("done")
    );
    let Some(detail) = failed else { return };
    if let Some(live) = lock(&shared.sessions).get_mut(&app.id) {
        live.session.launch_failed(now_ms());
        live.not_opened = true;
        live.problems.push(Problem {
            what: stfd_core::paths::file_name(&app.program).to_string(),
            code: "launch_failed".into(),
            detail,
        });
    }
}

fn launch_now(handle: &AppHandle, shared: &Arc<Shared>, app: &App) {
    // Already followed: open the program again, once a cleanup in progress has finished. A launch
    // still starting the services (the shortcut clicked twice) opens the program itself. Looking
    // and taking the session happen under one lock, so two clicks never both start the services.
    let started_waiting = now_ms();
    loop {
        let mut sessions = lock(&shared.sessions);
        match sessions.get_mut(&app.id) {
            None => {
                sessions.insert(
                    app.id.clone(),
                    Live {
                        session: Session::launched(&app.id, now_ms()),
                        problems: Vec::new(),
                        not_opened: false,
                        launching: true,
                    },
                );
                break;
            }
            Some(live) if live.launching => return,
            Some(live) if live.session.is_cleaning() => {
                drop(sessions);
                if now_ms() - started_waiting > 120_000 {
                    return;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            Some(live) => {
                live.session.relaunched(now_ms());
                drop(sessions);
                open_program(shared, app);
                return;
            }
        }
    }

    let snap = Snapshot {
        procs: Vec::new(),
        services: services::states().unwrap_or_default(),
    };
    let mut problems: Vec<Problem> = plan::missing_services(app, &snap)
        .into_iter()
        .map(|name| Problem {
            what: name.to_string(),
            code: "missing".into(),
            detail: String::new(),
        })
        .collect();
    let started = execute::start_services(&plan::to_start(app, &snap), &snap.services);
    if !started.problems.is_empty() {
        let names: Vec<String> = started.problems.iter().map(|p| p.what.clone()).collect();
        let (title, body) = lang::pick(&shared.settings().language).service_not_started(&app.name, &names);
        ui::notify(handle, shared, &title, &body);
    }
    problems.extend(started.problems);
    let any_started = !started.started.is_empty();
    if let Some(live) = lock(&shared.sessions).get_mut(&app.id) {
        live.session.started = started.started;
        live.problems.extend(problems);
    }
    if any_started && app.services_delay_seconds > 0 {
        std::thread::sleep(Duration::from_secs(app.services_delay_seconds.into()));
    }
    // The wait for the program starts now, not when the services started.
    if let Some(live) = lock(&shared.sessions).get_mut(&app.id) {
        live.session.relaunched(now_ms());
    }
    open_program(shared, app);
    if let Some(live) = lock(&shared.sessions).get_mut(&app.id) {
        live.launching = false;
    }
}

/// "Close leftovers now" for an app that is not open, or "Close now" while it waits.
pub fn close_now(handle: &AppHandle, shared: &Arc<Shared>, id: &str) -> Option<HistoryEntry> {
    {
        let mut sessions = lock(&shared.sessions);
        if let Some(live) = sessions.get_mut(id) {
            // Waiting after the program closed: clean up on the next step.
            live.session.close_now(now_ms());
            return None;
        }
        let mut session = Session::launched(id, now_ms());
        session.trigger = Trigger::Manual;
        session.phase = Phase::Cleaning;
        sessions.insert(
            id.to_string(),
            Live {
                session,
                problems: Vec::new(),
                not_opened: false,
                launching: false,
            },
        );
    }
    cleanup(handle, shared, id)
}

/// "Leave it running": no cleanup this time.
pub fn keep_running(shared: &Shared, id: &str) {
    let Some(live) = lock(&shared.sessions).remove(id) else {
        return;
    };
    let Some(app) = shared.app(id) else { return };
    let _ = shared.store.add_history(HistoryEntry {
        id: new_id(),
        app_id: app.id.clone(),
        app_name: app.name.clone(),
        trigger: live.session.trigger,
        outcome: Outcome::Kept,
        opened_at: live.session.opened_at,
        ended_at: now_ms(),
        started: live.session.started,
        stopped: Vec::new(),
        closed: Vec::new(),
        ended: Vec::new(),
        problems: live.problems,
    });
}
