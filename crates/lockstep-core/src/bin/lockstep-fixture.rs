//! A stand-in program for the Windows system tests (crates/lockstep-core/tests): it plays an app's
//! main program, a leftover helper, a stubborn program or a service.
//!
//!   lockstep-fixture window      a window that closes when asked
//!   lockstep-fixture stubborn    a window that ignores being asked to close
//!   lockstep-fixture background  no window, runs until ended
//!   lockstep-fixture service     a Windows service (run by the Service Control Manager)
//!   lockstep-fixture exit <n>    exits at once with code n
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("lockstep-fixture runs on Windows only");
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("window") => fixture::window(false),
        Some("stubborn") => fixture::window(true),
        Some("service") => fixture::service(),
        Some("exit") => std::process::exit(args.get(1).and_then(|c| c.parse().ok()).unwrap_or(0)),
        _ => loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        },
    }
}

#[cfg(windows)]
mod fixture {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows::core::{w, PWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, NO_ERROR, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Services::{
        RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW, SERVICE_ACCEPT_STOP,
        SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE,
        SERVICE_STOPPED, SERVICE_STOP_PENDING, SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassW,
        CW_USEDEFAULT, MSG, WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    };

    static STUBBORN: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_CLOSE if STUBBORN.load(Ordering::Relaxed) => LRESULT(0),
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }

    pub fn window(stubborn: bool) {
        STUBBORN.store(stubborn, Ordering::Relaxed);
        unsafe {
            let instance = GetModuleHandleW(None).expect("module handle");
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: w!("LockstepFixture"),
                ..Default::default()
            };
            RegisterClassW(&class);
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("LockstepFixture"),
                w!("Lockstep fixture"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                320,
                200,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .expect("window");
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
        }
    }

    static STATUS: AtomicIsize = AtomicIsize::new(0);
    static STOP: AtomicBool = AtomicBool::new(false);

    fn report(state: SERVICE_STATUS_CURRENT_STATE) {
        let status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: if state == SERVICE_RUNNING {
                SERVICE_ACCEPT_STOP
            } else {
                0
            },
            dwWaitHint: 3000,
            ..Default::default()
        };
        let handle = SERVICE_STATUS_HANDLE(STATUS.load(Ordering::SeqCst) as *mut _);
        unsafe {
            let _ = SetServiceStatus(handle, &status);
        }
    }

    unsafe extern "system" fn handler(
        control: u32,
        _: u32,
        _: *mut core::ffi::c_void,
        _: *mut core::ffi::c_void,
    ) -> u32 {
        if control == SERVICE_CONTROL_STOP {
            report(SERVICE_STOP_PENDING);
            STOP.store(true, Ordering::SeqCst);
        }
        NO_ERROR.0
    }

    unsafe extern "system" fn service_main(_: u32, _: *mut PWSTR) {
        let Ok(handle) = RegisterServiceCtrlHandlerExW(w!(""), Some(handler), None) else {
            return;
        };
        STATUS.store(handle.0 as isize, Ordering::SeqCst);
        report(SERVICE_RUNNING);
        while !STOP.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        report(SERVICE_STOPPED);
    }

    pub fn service() {
        // An own-process service may leave its name empty in the table.
        let mut name: Vec<u16> = vec![0];
        let table = [
            SERVICE_TABLE_ENTRYW {
                lpServiceName: PWSTR(name.as_mut_ptr()),
                lpServiceProc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW::default(),
        ];
        unsafe {
            let _ = StartServiceCtrlDispatcherW(table.as_ptr());
        }
    }
}
