// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The elevated step: Lockstep run as administrator to change service permissions, then exit.
    #[cfg(windows)]
    if args.first().map(String::as_str) == Some("--elevated") {
        std::process::exit(lockstep_core::windows::elevated::main(&args[1..]));
    }
    lockstep_lib::run(args);
}
