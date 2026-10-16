//! Opening programs: the app's program the way Explorer would, and STFD itself with
//! administrator rights for the one step that needs them.

use super::{wide, SysError, SysResult};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE};
use windows::Win32::System::Threading::{GetExitCodeProcess, GetProcessId, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// What happened when opening a program.
#[derive(Debug)]
pub enum Opened {
    /// It runs as this process (0 when Windows did not say, e.g. it handed over to a running copy).
    Pid(u32),
    /// The person said no to the administrator prompt.
    Cancelled,
}

fn execute(file: &str, args: &str, dir: Option<&str>, verb: Option<&str>) -> SysResult<Option<HANDLE>> {
    let file_w = wide(file);
    let args_w = wide(args);
    let dir_w = dir.map(wide);
    let verb_w = verb.map(wide);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: verb_w.as_ref().map_or(PCWSTR::null(), |v| PCWSTR(v.as_ptr())),
        lpFile: PCWSTR(file_w.as_ptr()),
        lpParameters: PCWSTR(args_w.as_ptr()),
        lpDirectory: dir_w.as_ref().map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        match ShellExecuteExW(&mut info) {
            Ok(()) => Ok((!info.hProcess.is_invalid()).then_some(info.hProcess)),
            Err(e) if e.code() == ERROR_CANCELLED.to_hresult() => Err(SysError::Other("cancelled".into())),
            Err(e) => Err(e.into()),
        }
    }
}

/// Opens a program the way double-clicking it would (Windows asks for administrator rights if the
/// program wants them), in its own folder.
pub fn open(program: &str, arguments: &str) -> SysResult<Opened> {
    let dir = crate::paths::parent(program);
    match execute(program, arguments, dir, None) {
        Ok(Some(handle)) => {
            let pid = unsafe { GetProcessId(handle) };
            unsafe {
                let _ = CloseHandle(handle);
            }
            Ok(Opened::Pid(pid))
        }
        Ok(None) => Ok(Opened::Pid(0)),
        Err(SysError::Other(m)) if m == "cancelled" => Ok(Opened::Cancelled),
        Err(e) => Err(e),
    }
}

/// Runs `exe` with administrator rights and waits for it; its exit code, or None when the person
/// said no.
pub fn run_elevated(exe: &str, arguments: &str) -> SysResult<Option<u32>> {
    let handle = match execute(exe, arguments, None, Some("runas")) {
        Ok(Some(h)) => h,
        Ok(None) => return Err(SysError::Other("Windows did not start the elevated step".into())),
        Err(SysError::Other(m)) if m == "cancelled" => return Ok(None),
        Err(e) => return Err(e),
    };
    unsafe {
        WaitForSingleObject(handle, INFINITE);
        let mut code = 0u32;
        let result = GetExitCodeProcess(handle, &mut code);
        let _ = CloseHandle(handle);
        result?;
        Ok(Some(code))
    }
}

/// True when this process runs with administrator rights.
pub fn is_elevated() -> bool {
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}
