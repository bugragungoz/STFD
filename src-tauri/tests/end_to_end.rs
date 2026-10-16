//! The built app, started the way its shortcut starts it (`lockstep --run <app>`): it opens the
//! program, follows it, and after the program closes ends the helper left running from its folder,
//! writes the session to History and exits. Ignored by default: it uses the real Lockstep config
//! folder of this account (saved and put back around the test) and the copy of `lockstep-fixture`
//! that `cargo test --workspace` builds next to the app. CI runs it on Windows.
#![cfg(windows)]

use lockstep_core::paths;
use lockstep_core::windows::process;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const APP: &str = env!("CARGO_BIN_EXE_lockstep");

fn fixture() -> PathBuf {
    let f = Path::new(APP).with_file_name("lockstep-fixture.exe");
    assert!(
        f.is_file(),
        "build lockstep-fixture first (cargo test --workspace builds it)"
    );
    f
}

/// Lockstep's folders for this account, as Tauri finds them.
fn dirs() -> (PathBuf, PathBuf) {
    let roaming = lockstep_core::windows::folders::all("").app_data[0].clone();
    let local = lockstep_core::windows::folders::all("").app_data[1].clone();
    (
        Path::new(&roaming).join("com.bugragungoz.lockstep"),
        Path::new(&local).join("com.bugragungoz.lockstep"),
    )
}

/// Moves a file aside for the test and puts it back after.
struct Aside(PathBuf);

impl Aside {
    fn new(path: PathBuf) -> Self {
        if path.exists() {
            let _ = std::fs::rename(&path, path.with_extension("e2e-saved"));
        }
        Aside(path)
    }
}

impl Drop for Aside {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let saved = self.0.with_extension("e2e-saved");
        if saved.exists() {
            let _ = std::fs::rename(saved, &self.0);
        }
    }
}

fn wait_until(what: &str, timeout: Duration, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn running(path: &str) -> Vec<lockstep_core::snapshot::Proc> {
    process::PathCache::default()
        .processes()
        .unwrap()
        .into_iter()
        .filter(|p| p.path.as_deref().is_some_and(|x| paths::same(x, path)))
        .collect()
}

#[test]
#[ignore = "runs the built app against this account's Lockstep folder; run with --include-ignored on Windows"]
fn the_shortcut_opens_the_program_and_lockstep_closes_what_it_left() {
    let temp = std::env::temp_dir().join(format!("lockstep-e2e-{}", lockstep_core::new_id()));
    let folder = temp.join("App");
    std::fs::create_dir_all(&folder).unwrap();
    let folder = lockstep_core::windows::long_path(&folder.to_string_lossy());
    let program = format!("{folder}\\app.exe");
    let helper = format!("{folder}\\helper.exe");
    std::fs::copy(fixture(), &program).unwrap();
    std::fs::copy(fixture(), &helper).unwrap();

    let (config, data) = dirs();
    std::fs::create_dir_all(&config).unwrap();
    std::fs::create_dir_all(&data).unwrap();
    let _apps = Aside::new(config.join("apps.json"));
    let _settings = Aside::new(config.join("settings.json"));
    let _history = Aside::new(data.join("history.json"));
    let app = serde_json::json!([{
        "id": "e2e",
        "name": "End to end",
        "program": program,
        "arguments": "window",
        "folders": [folder],
        "waitSeconds": 0,
        "servicesDelaySeconds": 0,
        "createdAt": 0
    }]);
    std::fs::write(config.join("apps.json"), app.to_string()).unwrap();
    std::fs::write(config.join("settings.json"), r#"{"notify":false,"politeSeconds":3}"#).unwrap();

    // A helper of the app left running from an earlier time.
    let mut left = Command::new(&helper).arg("background").spawn().unwrap();

    let mut lockstep = Command::new(APP).args(["--run", "e2e"]).spawn().unwrap();
    wait_until("the program's window", Duration::from_secs(60), || {
        let windowed = process::windowed_pids();
        running(&program).iter().any(|p| windowed.contains(&p.pid))
    });

    // The person closes the program.
    for p in running(&program) {
        process::ask_to_close(p.pid);
    }

    // Lockstep ends the helper, writes History and exits on its own.
    let start = Instant::now();
    let status = loop {
        if let Some(s) = lockstep.try_wait().unwrap() {
            break s;
        }
        if start.elapsed() > Duration::from_secs(90) {
            let _ = lockstep.kill();
            let _ = left.kill();
            panic!("Lockstep did not exit after the program closed");
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    assert!(status.success(), "{status:?}");
    assert!(left.try_wait().unwrap().is_some(), "the helper was ended");
    assert!(running(&program).is_empty() && running(&helper).is_empty());

    let history: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("history.json")).unwrap()).unwrap();
    let entry = &history[0];
    assert_eq!(entry["appId"], "e2e");
    assert_eq!(entry["trigger"], "shortcut");
    assert_eq!(entry["outcome"], "cleaned");
    assert_eq!(entry["ended"], serde_json::json!(["helper.exe"]));
    assert!(entry["openedAt"].is_u64());

    let _ = std::fs::remove_dir_all(&temp);
}
