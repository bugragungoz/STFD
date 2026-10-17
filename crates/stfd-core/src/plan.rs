//! What to start before an app opens and what to close after it, decided from a snapshot. The
//! Windows side carries the plan out; nothing here touches the system.

use crate::model::App;
use crate::paths::{self, KnownFolders};
use crate::snapshot::{Proc, Snapshot};
use serde::Serialize;

/// A program to close.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub windowed: bool,
}

impl Target {
    fn of(p: &Proc, path: &str) -> Self {
        Self {
            pid: p.pid,
            name: p.name.clone(),
            path: path.to_string(),
            windowed: p.windowed,
        }
    }
}

/// Why something of the app is left running.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Skip {
    /// It belongs to Windows.
    System,
    /// It runs for another person signed in to this computer.
    OtherUser,
    /// It is the process of a service the app does not stop; ending it would break the service.
    ServiceProcess { service: String },
    /// Another app that is open uses it.
    InUse { app: String },
    /// A running service outside the app needs it.
    DependentRunning { service: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// The program's file name or the service's display name.
    pub what: String,
    pub reason: Skip,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub programs: Vec<Target>,
    /// Services to stop (key names), the ones that need others first.
    pub services: Vec<String>,
    pub skipped: Vec<Skipped>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty() && self.services.is_empty()
    }
}

/// What the plan must know besides the app and the snapshot.
pub struct Context<'a> {
    /// STFD's own process, never closed.
    pub own_pid: u32,
    /// The user's Windows session; programs of other sessions are someone else's.
    pub own_session: u32,
    pub known: &'a KnownFolders,
    /// Other apps that are open now: what they use stays.
    pub others_open: &'a [App],
}

/// What to close after `app`: its programs still running and the services it stops.
pub fn cleanup(app: &App, snap: &Snapshot, ctx: &Context) -> Plan {
    let mut plan = Plan::default();

    // Services: the ones the app stops that are up, unless an open app uses them too.
    let mut stop: Vec<&crate::snapshot::Service> = Vec::new();
    for rule in app.services.iter().filter(|r| r.stop) {
        let Some(service) = snap.service(&rule.name).filter(|s| s.state.is_up()) else {
            continue;
        };
        if let Some(other) = ctx.others_open.iter().find(|o| o.service(&rule.name).is_some()) {
            plan.skipped.push(Skipped {
                what: service.display.clone(),
                reason: Skip::InUse {
                    app: other.name.clone(),
                },
            });
            continue;
        }
        stop.push(service);
    }

    // A service another running service needs cannot stop; drop those until none is left.
    let in_stop =
        |stop: &[&crate::snapshot::Service], name: &str| stop.iter().any(|s| s.name.eq_ignore_ascii_case(name));
    loop {
        let blocked = stop.iter().enumerate().find_map(|(i, s)| {
            snap.services
                .iter()
                .find(|d| d.state.is_up() && !in_stop(&stop, &d.name) && needs(d, &s.name))
                .map(|d| (i, d.display.clone()))
        });
        let Some((i, dependent)) = blocked else { break };
        let s = stop.remove(i);
        plan.skipped.push(Skipped {
            what: s.display.clone(),
            reason: Skip::DependentRunning { service: dependent },
        });
    }

    // Order: a service goes once nothing left in the list needs it.
    let mut left = stop.clone();
    while !left.is_empty() {
        let next = left
            .iter()
            .position(|s| {
                !left
                    .iter()
                    .any(|d| !d.name.eq_ignore_ascii_case(&s.name) && needs(d, &s.name))
            })
            .unwrap_or(0);
        plan.services.push(left.remove(next).name.clone());
    }

    if app.close_programs {
        for p in &snap.procs {
            let Some(path) = p.path.as_deref() else { continue };
            if p.pid == ctx.own_pid || !app.owns(path) {
                continue;
            }
            let skip = |reason| Skipped {
                what: p.name.clone(),
                reason,
            };
            if paths::is_system(path, ctx.known) {
                plan.skipped.push(skip(Skip::System));
                continue;
            }
            if p.session != 0 && p.session != ctx.own_session {
                plan.skipped.push(skip(Skip::OtherUser));
                continue;
            }
            // A service's process goes with the service, if the app stops all that live in it.
            let hosted: Vec<_> = snap.services_in(p.pid).collect();
            if let Some(kept) = hosted.iter().find(|s| !in_stop(&stop, &s.name)) {
                plan.skipped.push(skip(Skip::ServiceProcess {
                    service: kept.display.clone(),
                }));
                continue;
            }
            if !hosted.is_empty() {
                continue;
            }
            if let Some(other) = ctx.others_open.iter().find(|o| o.owns(path)) {
                plan.skipped.push(skip(Skip::InUse {
                    app: other.name.clone(),
                }));
                continue;
            }
            plan.programs.push(Target::of(p, path));
        }
        plan.programs.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.pid.cmp(&b.pid))
        });
    }
    plan
}

fn needs(service: &crate::snapshot::Service, name: &str) -> bool {
    service.depends_on.iter().any(|d| d.eq_ignore_ascii_case(name))
}

/// The services to start before the program opens, in the app's order: the ones that are not
/// running. Windows starts what they need on its own.
pub fn to_start(app: &App, snap: &Snapshot) -> Vec<String> {
    app.services
        .iter()
        .filter(|r| r.start)
        .filter(|r| snap.service(&r.name).is_some_and(|s| !s.state.is_up()))
        .map(|r| r.name.clone())
        .collect()
}

/// Services the app names that this Windows no longer has (uninstalled or renamed).
pub fn missing_services<'a>(app: &'a App, snap: &Snapshot) -> Vec<&'a str> {
    app.services
        .iter()
        .filter(|r| snap.service(&r.name).is_none())
        .map(|r| r.display.as_str())
        .collect()
}

/// The app's programs running now, as the window lists them.
pub fn running(app: &App, snap: &Snapshot, own_pid: u32) -> Vec<Target> {
    let mut out: Vec<Target> = snap
        .procs
        .iter()
        .filter(|p| p.pid != own_pid)
        .filter_map(|p| {
            let path = p.path.as_deref()?;
            (app.owns(path) || app.is_main(path)).then(|| Target::of(p, path))
        })
        .collect();
    out.sort_by(|a, b| {
        b.windowed
            .cmp(&a.windowed)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Programs to remember as seen with the app: its own, and the ones they started from other folders
/// (not Windows), which the person may choose to close as well.
pub fn seen(app: &App, snap: &Snapshot, known: &KnownFolders, own_pid: u32) -> Vec<String> {
    let mine: Vec<u32> = snap
        .procs
        .iter()
        .filter(|p| {
            p.path
                .as_deref()
                .is_some_and(|path| app.owns(path) || app.is_main(path))
        })
        .map(|p| p.pid)
        .collect();
    let mut out: Vec<String> = Vec::new();
    for p in &snap.procs {
        let Some(path) = p.path.as_deref() else { continue };
        if p.pid == own_pid || paths::is_system(path, known) || app.is_kept(path) {
            continue;
        }
        let own = mine.contains(&p.pid);
        let child = p.parent != 0 && mine.contains(&p.parent);
        if (own || child) && !out.iter().any(|o| paths::same(o, path)) {
            out.push(path.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{App, ServiceRule};
    use crate::snapshot::{Service, SvcState};

    const LAB: &str = "C:\\Program Files (x86)\\Example Labs";

    fn known() -> KnownFolders {
        KnownFolders {
            windows: "C:\\Windows".into(),
            ..KnownFolders::default()
        }
    }

    fn rule(name: &str) -> ServiceRule {
        ServiceRule {
            name: name.into(),
            display: format!("{name} display"),
            start: true,
            stop: true,
            original: Default::default(),
        }
    }

    fn app(services: &[&str]) -> App {
        App {
            id: "ms".into(),
            name: "Circuit Simulator".into(),
            program: format!("{LAB}\\Circuit Simulator 14\\circuitsim.exe"),
            arguments: String::new(),
            folders: vec![LAB.into()],
            services: services.iter().map(|s| rule(s)).collect(),
            also_close: vec![],
            keep: vec![],
            wait_seconds: None,
            services_delay_seconds: 3,
            close_programs: true,
            shortcuts: Default::default(),
            seen: vec![],
            created_at: 0,
        }
    }

    fn proc(pid: u32, parent: u32, path: &str) -> Proc {
        Proc {
            pid,
            parent,
            name: paths::file_name(path).to_string(),
            path: Some(path.to_string()),
            windowed: false,
            session: 1,
        }
    }

    fn svc(name: &str, state: SvcState, pid: u32, depends_on: &[&str]) -> Service {
        Service {
            name: name.into(),
            display: format!("{name} display"),
            state,
            pid,
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn ctx<'a>(known: &'a KnownFolders, others: &'a [App]) -> Context<'a> {
        Context {
            own_pid: 99,
            own_session: 1,
            known,
            others_open: others,
        }
    }

    #[test]
    fn leftover_programs_of_the_folders_close_and_others_stay() {
        let k = known();
        let snap = Snapshot {
            procs: vec![
                proc(10, 1, &format!("{LAB}\\Shared\\elsvcloc.exe")),
                proc(11, 1, &format!("{LAB}\\Shared\\ELUpdate.exe")),
                proc(12, 1, "C:\\Program Files\\Other\\other.exe"),
                proc(99, 1, &format!("{LAB}\\stfd-here.exe")),
                Proc {
                    path: None,
                    ..proc(13, 1, "x")
                },
            ],
            services: vec![],
        };
        let plan = cleanup(&app(&[]), &snap, &ctx(&k, &[]));
        let names: Vec<_> = plan.programs.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["elsvcloc.exe", "ELUpdate.exe"]);
        assert!(plan.services.is_empty() && plan.skipped.is_empty());
    }

    #[test]
    fn programs_are_left_alone_when_the_app_does_not_close_them() {
        let k = known();
        let mut a = app(&[]);
        a.close_programs = false;
        let snap = Snapshot {
            procs: vec![proc(10, 1, &format!("{LAB}\\Shared\\x.exe"))],
            services: vec![],
        };
        assert!(cleanup(&a, &snap, &ctx(&k, &[])).is_empty());
    }

    #[test]
    fn windows_programs_other_peoples_programs_and_kept_ones_stay() {
        let k = known();
        let mut a = app(&[]);
        a.also_close = vec!["C:\\Windows\\System32\\notepad.exe".into()];
        a.keep = vec![format!("{LAB}\\Explorer\\hwexplorer.exe")];
        let snap = Snapshot {
            procs: vec![
                proc(10, 1, "C:\\Windows\\System32\\notepad.exe"),
                Proc {
                    session: 2,
                    ..proc(11, 1, &format!("{LAB}\\Shared\\x.exe"))
                },
                proc(12, 1, &format!("{LAB}\\Explorer\\hwexplorer.exe")),
            ],
            services: vec![],
        };
        let plan = cleanup(&a, &snap, &ctx(&k, &[]));
        assert!(plan.programs.is_empty());
        assert_eq!(
            plan.skipped,
            vec![
                Skipped {
                    what: "notepad.exe".into(),
                    reason: Skip::System
                },
                Skipped {
                    what: "x.exe".into(),
                    reason: Skip::OtherUser
                },
            ]
        );
    }

    #[test]
    fn running_services_stop_after_the_ones_that_need_them() {
        let k = known();
        let a = app(&["ELDomainService", "ELAuthService", "ELStopped", "ELNotInstalled"]);
        let snap = Snapshot {
            procs: vec![],
            services: vec![
                svc("ELDomainService", SvcState::Running, 20, &[]),
                svc("ELAuthService", SvcState::Running, 21, &["eldomainservice"]),
                svc("ELStopped", SvcState::Stopped, 0, &[]),
            ],
        };
        let plan = cleanup(&a, &snap, &ctx(&k, &[]));
        assert_eq!(plan.services, vec!["ELAuthService", "ELDomainService"]);
        assert!(plan.skipped.is_empty());
        assert_eq!(missing_services(&a, &snap), vec!["ELNotInstalled display"]);
    }

    #[test]
    fn a_service_another_running_service_needs_stays_and_so_does_what_it_needs() {
        let k = known();
        let a = app(&["Base", "Middle"]);
        let snap = Snapshot {
            procs: vec![],
            services: vec![
                svc("Base", SvcState::Running, 20, &[]),
                svc("Middle", SvcState::Running, 21, &["Base"]),
                svc("Outside", SvcState::Running, 22, &["Middle"]),
            ],
        };
        let plan = cleanup(&a, &snap, &ctx(&k, &[]));
        assert!(plan.services.is_empty());
        assert_eq!(
            plan.skipped,
            vec![
                Skipped {
                    what: "Middle display".into(),
                    reason: Skip::DependentRunning {
                        service: "Outside display".into()
                    }
                },
                Skipped {
                    what: "Base display".into(),
                    reason: Skip::DependentRunning {
                        service: "Middle display".into()
                    }
                },
            ]
        );
    }

    #[test]
    fn what_another_open_app_uses_stays() {
        let k = known();
        let a = app(&["ELDomainService"]);
        let mut bench = app(&["ELDomainService"]);
        bench.name = "Bench".into();
        let snap = Snapshot {
            procs: vec![proc(10, 1, &format!("{LAB}\\Shared\\x.exe"))],
            services: vec![svc("ELDomainService", SvcState::Running, 20, &[])],
        };
        let others = [bench];
        let plan = cleanup(&a, &snap, &ctx(&k, &others));
        assert!(plan.is_empty());
        assert_eq!(plan.skipped.len(), 2);
        assert!(plan
            .skipped
            .iter()
            .all(|s| s.reason == Skip::InUse { app: "Bench".into() }));
    }

    #[test]
    fn a_service_process_goes_with_its_services_or_stays_with_them() {
        let k = known();
        let host = format!("{LAB}\\Shared\\elsvchost.exe");
        let snap = Snapshot {
            procs: vec![proc(20, 1, &host)],
            services: vec![
                svc("A", SvcState::Running, 20, &[]),
                svc("B", SvcState::Running, 20, &[]),
            ],
        };
        // Both services stop: the process ends with them, not by hand.
        let plan = cleanup(&app(&["A", "B"]), &snap, &ctx(&k, &[]));
        assert!(plan.programs.is_empty() && plan.skipped.is_empty());
        assert_eq!(plan.services, vec!["A", "B"]);
        // B is not the app's: ending the process would kill B.
        let plan = cleanup(&app(&["A"]), &snap, &ctx(&k, &[]));
        assert!(plan.programs.is_empty());
        assert_eq!(
            plan.skipped,
            vec![Skipped {
                what: "elsvchost.exe".into(),
                reason: Skip::ServiceProcess {
                    service: "B display".into()
                }
            }]
        );
    }

    #[test]
    fn services_start_in_the_apps_order_when_they_are_not_running() {
        let mut a = app(&["One", "Two", "Three"]);
        a.services[2].start = false;
        let snap = Snapshot {
            procs: vec![],
            services: vec![
                svc("Two", SvcState::Stopped, 0, &[]),
                svc("One", SvcState::Running, 5, &[]),
                svc("Three", SvcState::Stopped, 0, &[]),
            ],
        };
        assert_eq!(to_start(&a, &snap), vec!["Two"]);
    }

    #[test]
    fn running_lists_windows_first_and_seen_adds_children_from_elsewhere() {
        let k = known();
        let a = app(&[]);
        let main = Proc {
            windowed: true,
            ..proc(10, 1, &a.program)
        };
        let snap = Snapshot {
            procs: vec![
                proc(11, 10, &format!("{LAB}\\Shared\\helper.exe")),
                main,
                proc(12, 10, "C:\\Users\\b\\AppData\\Local\\Crash\\reporter.exe"),
                proc(13, 10, "C:\\Windows\\System32\\conhost.exe"),
                proc(14, 1, "C:\\Program Files\\Other\\other.exe"),
            ],
            services: vec![],
        };
        let names: Vec<_> = running(&a, &snap, 99).into_iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["circuitsim.exe", "helper.exe"]);
        assert_eq!(
            seen(&a, &snap, &k, 99),
            vec![
                format!("{LAB}\\Shared\\helper.exe"),
                a.program.clone(),
                "C:\\Users\\b\\AppData\\Local\\Crash\\reporter.exe".to_string(),
            ]
        );
    }
}
