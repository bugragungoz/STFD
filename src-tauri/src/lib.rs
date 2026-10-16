//! The Lockstep app: started by a shortcut (`--run <app>`) it opens that app and follows it
//! without a window; started plainly it shows its window; `--background` (at sign-in, when it
//! watches apps opened another way) starts it in the tray. A second start hands its request to the
//! first. Everything about apps and Windows lives in `lockstep-core`; this crate wires it to Tauri.

// Most of the shell only does something on Windows.
#![cfg_attr(not(windows), allow(dead_code))]

/// What Lockstep does, on stderr, when LOCKSTEP_LOG is set (the end-to-end test sets it). Release
/// builds have no console, so it costs nothing there.
macro_rules! trace {
    ($($arg:tt)*) => {
        if std::env::var_os("LOCKSTEP_LOG").is_some() {
            eprintln!("lockstep: {}", format!($($arg)*));
        }
    };
}

mod commands;
mod lang;
#[cfg(windows)]
mod setup;
mod state;
mod ui;
#[cfg(windows)]
mod watcher;

use state::Shared;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Start {
    Window,
    Run(String),
    Background,
}

fn parse(args: &[String]) -> Start {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--run" => {
                if let Some(id) = it.next() {
                    return Start::Run(id.clone());
                }
            }
            "--background" => return Start::Background,
            _ => {}
        }
    }
    Start::Window
}

fn handle(app: &AppHandle, start: Start) {
    match start {
        Start::Window => ui::show_window(app),
        #[cfg(windows)]
        Start::Run(id) => watcher::launch(app, &app.state::<Arc<Shared>>(), &id),
        #[cfg(not(windows))]
        Start::Run(_) => ui::show_window(app),
        Start::Background => {}
    }
}

pub fn run(args: Vec<String>) {
    let start = parse(&args);
    trace!("start {start:?}");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            handle(app, parse(argv.get(1..).unwrap_or(&[])));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let config = app.path().app_config_dir()?;
            let data = app.path().app_local_data_dir()?;
            let own_exe = std::env::current_exe()?.to_string_lossy().to_string();
            #[cfg(windows)]
            let known = lockstep_core::windows::folders::all(lockstep_core::paths::parent(&own_exe).unwrap_or(""));
            #[cfg(not(windows))]
            let known = lockstep_core::paths::KnownFolders::default();
            let shared = Arc::new(Shared::new(
                lockstep_core::store::Store::new(config, data),
                known,
                own_exe,
            ));
            app.manage(shared.clone());
            // Without a tray (a session with no taskbar) Lockstep still works.
            let _ = ui::create_tray(app.handle());
            #[cfg(windows)]
            setup::sync_autostart(&shared);
            handle(app.handle(), start.clone());
            #[cfg(windows)]
            watcher::start(app.handle().clone(), shared);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    window.state::<Arc<Shared>>().window_open.store(false, Ordering::SeqCst);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_icon,
            commands::get_settings,
            commands::set_settings,
            commands::get_history,
            commands::clear_history,
            commands::inspect_folder,
            commands::inspect_program,
            commands::list_services,
            commands::find_shortcuts,
            commands::save_app,
            commands::update_app,
            commands::grant_permission,
            commands::remove_app,
            commands::open_app,
            commands::preview_cleanup,
            commands::close_now,
            commands::keep_running,
            commands::make_shortcuts,
        ])
        .build(tauri::generate_context!())
        .expect("Lockstep could not start")
        .run(|app, event| {
            // Closing the window does not end Lockstep while it follows an app.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                trace!("exit requested ({code:?})");
                if code.is_none() && app.state::<Arc<Shared>>().busy() {
                    api.prevent_exit();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_command_line_picks_how_lockstep_starts() {
        assert_eq!(parse(&args(&[])), Start::Window);
        assert_eq!(parse(&args(&["--run", "abc123"])), Start::Run("abc123".into()));
        assert_eq!(parse(&args(&["--background"])), Start::Background);
        assert_eq!(parse(&args(&["--run"])), Start::Window, "no app named");
        assert_eq!(parse(&args(&["--something", "--run", "x"])), Start::Run("x".into()));
    }
}
