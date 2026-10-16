//! Lockstep against a real Windows: processes and windows, closing leftovers politely and by force,
//! a real service started, stopped and given permissions, shortcuts, program details and the
//! elevated step. They change the system (a test service, the Run key) and need administrator
//! rights, so they are ignored by default; CI runs them on a Windows runner (which is elevated)
//! with `--include-ignored`. The programs they run are copies of `lockstep-fixture`.
#![cfg(windows)]

use lockstep_core::elevated::{Change, Outcome, Request};
use lockstep_core::model::{App, ServiceRule, StartType};
use lockstep_core::paths::{self, KnownFolders};
use lockstep_core::plan::{self, Context};
use lockstep_core::snapshot::{Snapshot, SvcState};
use lockstep_core::windows::{
    autostart, elevated, execute, folders, icon, launch, process, services, shortcut, version,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const FIXTURE: &str = env!("CARGO_BIN_EXE_lockstep-fixture");

/// A fresh folder under the temp folder, removed at the end.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("lockstep-{name}-{}", lockstep_core::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }

    /// A copy of the fixture at `relative` (`App\app.exe`).
    fn program(&self, relative: &str) -> String {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(FIXTURE, &path).unwrap();
        path.to_string_lossy().to_string()
    }

    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().to_string()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Ends a started program at the end of a test, however it went.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run(program: &str, mode: &str) -> Running {
    Running(Command::new(program).arg(mode).spawn().unwrap())
}

fn wait_until(what: &str, timeout: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn app(folder: &str, program: &str) -> App {
    App {
        id: "test".into(),
        name: "Test app".into(),
        program: program.into(),
        arguments: String::new(),
        folders: vec![folder.into()],
        services: vec![],
        also_close: vec![],
        keep: vec![],
        wait_seconds: Some(0),
        services_delay_seconds: 0,
        close_programs: true,
        shortcuts: Default::default(),
        seen: vec![],
        created_at: 0,
    }
}

fn snapshot(cache: &mut process::PathCache) -> Snapshot {
    Snapshot {
        procs: cache.processes().unwrap(),
        services: services::states_with_dependencies().unwrap(),
    }
}

fn known() -> KnownFolders {
    folders::all("")
}

#[test]
#[ignore = "reads the system; run with --include-ignored on Windows"]
fn program_details_icons_and_folders() {
    let cmd = std::env::var("ComSpec").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".into());
    let v = version::strings(&cmd);
    assert_eq!(v.company.as_deref(), Some("Microsoft Corporation"));
    assert_eq!(v.description.as_deref(), Some("Windows Command Processor"));

    let png = icon::png(&cmd, 32).expect("cmd.exe has an icon");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    let k = known();
    assert!(k.windows.to_lowercase().ends_with("windows"), "{}", k.windows);
    assert_eq!(k.program_files.len(), 2);
    assert!(folders::desktop().is_some() && folders::start_menu().is_some());
    assert!(paths::check_folder("C:\\Program Files", &k).is_err());
    assert!(paths::is_system(&cmd, &k));
}

#[test]
#[ignore = "reads the system; run with --include-ignored on Windows"]
fn the_process_list_has_this_process_with_its_program_and_session() {
    let mut cache = process::PathCache::default();
    let procs = cache.processes().unwrap();
    let me = procs
        .iter()
        .find(|p| p.pid == process::own_pid())
        .expect("this process");
    let exe = std::env::current_exe().unwrap();
    assert!(paths::same(me.path.as_deref().unwrap(), &exe.to_string_lossy()));
    assert_eq!(me.session, process::session_of(process::own_pid()));
    // A second read uses the cache and agrees.
    let again = cache.processes().unwrap();
    assert_eq!(again.iter().find(|p| p.pid == me.pid).unwrap().path, me.path);
}

#[test]
#[ignore = "opens windows; run with --include-ignored on Windows"]
fn an_opened_program_shows_a_window_and_closes_when_asked() {
    let temp = Temp::new("window");
    let program = temp.program("App\\app.exe");
    let pid = match launch::open(&program, "window").unwrap() {
        launch::Opened::Pid(pid) => pid,
        other => panic!("{other:?}"),
    };
    assert_ne!(pid, 0);
    wait_until("the window", Duration::from_secs(15), || {
        process::windowed_pids().contains(&pid)
    });

    let a = app(&temp.path("App"), &program);
    let mut cache = process::PathCache::default();
    assert!(a.is_open(&cache.processes().unwrap(), process::own_pid()));

    let held = process::Held::open(pid, &program).unwrap();
    assert!(process::ask_to_close(pid) >= 1);
    assert!(held.wait(15_000), "it closed");
    assert!(!a.is_open(&cache.processes().unwrap(), process::own_pid()));
    assert!(process::Held::open(pid, &program).is_err(), "gone");
}

#[test]
#[ignore = "starts and ends programs; run with --include-ignored on Windows"]
fn leftovers_close_when_asked_or_are_ended_and_other_programs_stay() {
    let temp = Temp::new("leftovers");
    let main = temp.program("App\\app.exe");
    let polite = temp.program("App\\polite.exe");
    let helper = temp.program("App\\bin\\helper.exe");
    let stubborn = temp.program("App\\stubborn.exe");
    let other = temp.program("Other\\other.exe");
    let _polite = run(&polite, "window");
    let _helper = run(&helper, "background");
    let _stubborn = run(&stubborn, "stubborn");
    let _other = run(&other, "background");

    let a = app(&temp.path("App"), &main);
    let mut cache = process::PathCache::default();
    wait_until("the windows", Duration::from_secs(15), || {
        let windowed = process::windowed_pids();
        windowed.contains(&_polite.0.id()) && windowed.contains(&_stubborn.0.id())
    });
    let snap = snapshot(&mut cache);
    // Windows of the app's programs count as the app being open, even without its main program.
    assert!(a.is_open(&snap.procs, process::own_pid()));

    let k = known();
    let ctx = Context {
        own_pid: process::own_pid(),
        own_session: process::session_of(process::own_pid()),
        known: &k,
        others_open: &[],
    };
    let plan = plan::cleanup(&a, &snap, &ctx);
    let names: Vec<_> = plan.programs.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["helper.exe", "polite.exe", "stubborn.exe"]);

    let started = Instant::now();
    let done = execute::close(&plan, &snap.services, Duration::from_secs(3), || {
        let snap = Snapshot {
            procs: process::PathCache::default().processes().unwrap(),
            services: vec![],
        };
        plan::cleanup(&a, &snap, &ctx).programs
    });
    assert_eq!(done.closed, vec!["polite.exe"]);
    assert_eq!(done.ended, vec!["helper.exe", "stubborn.exe"]);
    assert!(done.problems.is_empty(), "{:?}", done.problems);
    assert!(
        started.elapsed() >= Duration::from_secs(3),
        "the stubborn one got its time"
    );

    let procs = process::PathCache::default().processes().unwrap();
    assert!(
        !procs.iter().any(|p| p.path.as_deref().is_some_and(|x| a.owns(x))),
        "nothing of the app is left"
    );
    assert!(
        procs.iter().any(|p| p.pid == _other.0.id()),
        "the other program still runs"
    );
}

/// A test service running a copy of the fixture, deleted at the end.
struct TestService {
    name: String,
}

impl TestService {
    fn create(program: &str) -> Self {
        let name = format!("LockstepTest{}", lockstep_core::new_id());
        let ok = Command::new("sc.exe")
            .args([
                "create",
                &name,
                "binPath=",
                &format!("\"{program}\" service"),
                "start=",
                "demand",
                "DisplayName=",
                &format!("{name} display"),
            ])
            .status()
            .unwrap()
            .success();
        assert!(ok, "sc create");
        TestService { name }
    }
}

impl Drop for TestService {
    fn drop(&mut self) {
        let _ = services::stop(&self.name, Duration::from_secs(10));
        let _ = Command::new("sc.exe").args(["delete", &self.name]).status();
    }
}

#[test]
#[ignore = "creates a service and needs administrator rights; run with --include-ignored on Windows"]
fn a_service_of_the_app_starts_before_it_and_stops_after_it() {
    let temp = Temp::new("service");
    let svc_program = temp.program("Vendor\\Shared\\svc.exe");
    let main = temp.program("Vendor\\App\\app.exe");
    let svc = TestService::create(&svc_program);
    let k = known();

    // Listed with its program, found as part of the app's folder.
    let all = services::list(&k).unwrap();
    let info = all.iter().find(|s| s.name == svc.name).expect("listed");
    assert!(paths::same(info.program.as_deref().unwrap(), &svc_program));
    assert_eq!(info.start_type, StartType::Manual);
    assert_eq!(info.state, SvcState::Stopped);
    assert!(info.controllable, "an administrator may start and stop it");
    let related = lockstep_core::services::related(&all, &[temp.path("Vendor")], None, &k);
    assert!(related.iter().any(|r| r.service.name == svc.name));

    let mut a = app(&temp.path("Vendor"), &main);
    a.services.push(ServiceRule {
        name: svc.name.clone(),
        display: format!("{} display", svc.name),
        start: true,
        stop: true,
        original: Default::default(),
    });

    // Before the program opens.
    let mut cache = process::PathCache::default();
    let snap = snapshot(&mut cache);
    let names = plan::to_start(&a, &snap);
    assert_eq!(names, vec![svc.name.clone()]);
    let started = execute::start_services(&names, &snap.services);
    assert_eq!(started.started, vec![format!("{} display", svc.name)]);
    assert!(started.problems.is_empty(), "{:?}", started.problems);
    let snap = snapshot(&mut cache);
    let state = snap.service(&svc.name).unwrap();
    assert_eq!(state.state, SvcState::Running);
    assert_ne!(state.pid, 0);
    assert!(plan::to_start(&a, &snap).is_empty(), "running already");

    // After it closed: the service stops, its process goes with it and is not ended by hand.
    let ctx = Context {
        own_pid: process::own_pid(),
        own_session: process::session_of(process::own_pid()),
        known: &k,
        others_open: &[],
    };
    let plan = plan::cleanup(&a, &snap, &ctx);
    assert_eq!(plan.services, vec![svc.name.clone()]);
    assert!(plan.programs.is_empty(), "{:?}", plan.programs);
    assert!(plan.skipped.is_empty(), "{:?}", plan.skipped);
    let done = execute::close(&plan, &snap.services, Duration::from_secs(5), Vec::new);
    assert_eq!(done.stopped, vec![format!("{} display", svc.name)]);
    assert!(done.problems.is_empty(), "{:?}", done.problems);
    assert_eq!(
        snapshot(&mut cache).service(&svc.name).unwrap().state,
        SvcState::Stopped
    );

    // Another open app that uses the service keeps it running.
    services::start(&svc.name, Duration::from_secs(30)).unwrap();
    let mut other = a.clone();
    other.name = "Other app".into();
    let others = [other];
    let ctx = Context {
        others_open: &others,
        ..ctx
    };
    let plan = plan::cleanup(&a, &snapshot(&mut cache), &ctx);
    assert!(plan.services.is_empty());
    services::stop(&svc.name, Duration::from_secs(30)).unwrap();
}

#[test]
#[ignore = "changes a service's permissions and needs administrator rights; run with --include-ignored on Windows"]
fn the_elevated_step_gives_and_takes_back_permission_and_the_start_type() {
    let temp = Temp::new("rights");
    let svc = TestService::create(&temp.program("Svc\\svc.exe"));
    let sid = services::own_sid().unwrap();
    assert!(lockstep_core::sddl::is_account_sid(&sid), "{sid}");
    let before = services::sddl(&svc.name).unwrap();
    assert!(!lockstep_core::sddl::has_entry(&before, &sid));
    services::set_start_type(&svc.name, StartType::Automatic).unwrap();

    // Through the command line, as the app runs it.
    let request = Request {
        sid: sid.clone(),
        services: vec![Change {
            name: svc.name.clone(),
            grant: true,
            manual: true,
            ..Change::default()
        }],
    };
    let result = temp.path("result.json");
    let code = elevated::main(&[lockstep_core::elevated::encode(&request), result.clone()]);
    assert_eq!(code, 0);
    let outcomes: Vec<Outcome> = lockstep_core::store::read(Path::new(&result));
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].error, None);
    assert!(outcomes[0].original.granted);
    assert_eq!(outcomes[0].original.start_type, Some(StartType::Automatic));
    assert!(lockstep_core::sddl::has_entry(
        &services::sddl(&svc.name).unwrap(),
        &sid
    ));
    assert_eq!(services::get_start_type(&svc.name).unwrap(), StartType::Manual);

    // Granting again changes nothing; taking it back leaves the rest as it was.
    let again = elevated::run(&request);
    assert_eq!(again[0].error, None);
    assert_eq!(again[0].original.start_type, None, "already manual");
    let undo = Request {
        sid: sid.clone(),
        services: vec![Change {
            name: svc.name.clone(),
            revoke: true,
            restore_start: Some(StartType::Automatic),
            ..Change::default()
        }],
    };
    assert_eq!(elevated::run(&undo)[0].error, None);
    let after = services::sddl(&svc.name).unwrap();
    assert!(!lockstep_core::sddl::has_entry(&after, &sid));
    assert_eq!(after, before);
    assert_eq!(services::get_start_type(&svc.name).unwrap(), StartType::Automatic);

    // A request for a group, not a person, is refused before anything changes.
    let bad = Request {
        sid: "S-1-1-0".into(),
        ..request
    };
    assert_eq!(elevated::main(&[lockstep_core::elevated::encode(&bad), result]), 2);
}

#[test]
#[ignore = "runs a program elevated; run with --include-ignored on a Windows runner without UAC prompts"]
fn a_program_runs_elevated_and_its_exit_code_comes_back() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(launch::run_elevated(FIXTURE, "exit 7"));
    });
    let result = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("no answer: Windows is probably showing an administrator prompt");
    assert_eq!(result.unwrap(), Some(7));
    assert!(launch::is_elevated(), "CI runs elevated");
}

#[test]
#[ignore = "writes files; run with --include-ignored on Windows"]
fn a_shortcut_is_written_read_back_and_found() {
    let temp = Temp::new("shortcut");
    let target = temp.program("App\\app.exe");
    let link = temp.path(&paths::shortcut_file_name("Test: app"));
    shortcut::create(
        &link,
        &shortcut::Link {
            target: FIXTURE,
            arguments: "--run abc123",
            working_dir: &temp.path("App"),
            icon: &target,
            description: "Opens the test app with Lockstep",
        },
    )
    .unwrap();
    assert!(link.ends_with("Test app.lnk"), "{link}");
    let (opens, args) = shortcut::read(&link).unwrap();
    assert!(paths::same(&opens, FIXTURE));
    assert_eq!(args, "--run abc123");

    // The program's own shortcut is found; Lockstep's (which opens Lockstep) is not.
    let direct = temp.path("Direct.lnk");
    shortcut::create(
        &direct,
        &shortcut::Link {
            target: &target,
            arguments: "",
            working_dir: "",
            icon: &target,
            description: "",
        },
    )
    .unwrap();
    assert_eq!(
        shortcut::find_for(&target, &[temp.0.to_string_lossy().to_string()]),
        vec![direct]
    );
}

#[test]
#[ignore = "changes the Run key; run with --include-ignored on Windows"]
fn starting_with_windows_can_be_set_and_removed() {
    let before = autostart::get();
    autostart::set("\"C:\\Lockstep\\lockstep.exe\" --background").unwrap();
    assert_eq!(
        autostart::get().as_deref(),
        Some("\"C:\\Lockstep\\lockstep.exe\" --background")
    );
    autostart::remove();
    assert_eq!(autostart::get(), None);
    if let Some(b) = before {
        autostart::set(&b).unwrap();
    }
}
