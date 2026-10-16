//! Processes: the list with their programs and windows, asking one to close, and ending it.

use super::{from_wide, SysError, SysResult};
use crate::paths;
use crate::snapshot::Proc;
use std::collections::{HashMap, HashSet};
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_ACCESS_RIGHTS, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
    PostMessageW, GWL_EXSTYLE, GW_OWNER, WM_CLOSE, WS_EX_TOOLWINDOW,
};

pub fn own_pid() -> u32 {
    unsafe { GetCurrentProcessId() }
}

pub fn session_of(pid: u32) -> u32 {
    let mut session = 0u32;
    unsafe {
        let _ = ProcessIdToSessionId(pid, &mut session);
    }
    session
}

fn path_of_handle(handle: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut len = buffer.len() as u32;
    unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len).ok()? };
    Some(String::from_utf16_lossy(&buffer[..len as usize]))
}

/// The full path of a process's program; None for the System process, one that ended, or a
/// protected one.
pub fn program_path(pid: u32) -> Option<String> {
    if pid == 0 || pid == 4 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let path = path_of_handle(handle);
        let _ = CloseHandle(handle);
        path
    }
}

/// Program paths of processes seen before, so each step reads only the new ones.
#[derive(Default)]
pub struct PathCache {
    known: HashMap<u32, (String, u32, Option<String>, u32)>,
}

impl PathCache {
    /// Every process, with its program, Windows session and whether it shows a window.
    pub fn processes(&mut self) -> SysResult<Vec<Proc>> {
        let windowed = windowed_pids();
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snap, &mut entry).is_ok();
            while ok {
                let pid = entry.th32ProcessID;
                let parent = entry.th32ParentProcessID;
                let name = from_wide(&entry.szExeFile);
                seen.insert(pid);
                // The same pid with the same name and parent is the same process.
                let cached = self
                    .known
                    .get(&pid)
                    .filter(|(n, p, _, _)| *n == name && *p == parent)
                    .map(|(_, _, path, session)| (path.clone(), *session));
                let (path, session) = cached.unwrap_or_else(|| {
                    let found = (program_path(pid), session_of(pid));
                    self.known.insert(pid, (name.clone(), parent, found.0.clone(), found.1));
                    found
                });
                out.push(Proc {
                    pid,
                    parent,
                    name,
                    path,
                    windowed: windowed.contains(&pid),
                    session,
                });
                ok = Process32NextW(snap, &mut entry).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        self.known.retain(|pid, _| seen.contains(pid));
        Ok(out)
    }
}

/// Visits every top-level window.
fn each_window(mut f: impl FnMut(HWND)) {
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let f = &mut *(data.0 as *mut &mut dyn FnMut(HWND));
        f(hwnd);
        BOOL(1)
    }
    let mut dynf: &mut dyn FnMut(HWND) = &mut f;
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut dynf as *mut _ as isize));
    }
}

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

/// Processes that show a window a person would see: visible, not owned by another window, not a
/// tool window, with a size.
pub fn windowed_pids() -> HashSet<u32> {
    let mut out = HashSet::new();
    each_window(|hwnd| unsafe {
        if !IsWindowVisible(hwnd).as_bool() || GetWindow(hwnd, GW_OWNER).is_ok() {
            return;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() || rect.right - rect.left <= 0 || rect.bottom - rect.top <= 0 {
            return;
        }
        out.insert(pid_of(hwnd));
    });
    out
}

/// Asks a process to close the way its close button would: WM_CLOSE to each of its top-level
/// windows, hidden ones too (tray programs keep one), except input-method windows. Returns how
/// many windows were asked.
pub fn ask_to_close(pid: u32) -> usize {
    let mut asked = 0;
    each_window(|hwnd| unsafe {
        if pid_of(hwnd) != pid {
            return;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        if class == "IME" || class == "MSCTFIME UI" {
            return;
        }
        if PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).is_ok() {
            asked += 1;
        }
    });
    asked
}

/// A process held open while STFD closes it, so its number cannot be reused by another
/// process in the meantime.
pub struct Held {
    pub pid: u32,
    pub name: String,
    handle: HANDLE,
    can_end: bool,
}

impl Held {
    /// Opens the process if it still runs `expected` (its program path).
    pub fn open(pid: u32, expected: &str) -> SysResult<Held> {
        unsafe {
            let full = PROCESS_TERMINATE | PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION;
            let (handle, can_end) = match OpenProcess(full, false, pid) {
                Ok(h) => (h, true),
                // An elevated or another account's process: it can still be watched, not ended.
                Err(_) => (
                    OpenProcess(
                        PROCESS_ACCESS_RIGHTS(PROCESS_SYNCHRONIZE.0 | PROCESS_QUERY_LIMITED_INFORMATION.0),
                        false,
                        pid,
                    )?,
                    false,
                ),
            };
            let held = Held {
                pid,
                name: paths::file_name(expected).to_string(),
                handle,
                can_end,
            };
            match path_of_handle(handle) {
                Some(path) if paths::same(&path, expected) => Ok(held),
                _ => Err(SysError::Other("the process is not the same program any more".into())),
            }
        }
    }

    pub fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
    }

    /// Ends the process at once.
    pub fn end(&self) -> SysResult<()> {
        if !self.can_end {
            return Err(SysError::AccessDenied);
        }
        unsafe { TerminateProcess(self.handle, 1)? };
        Ok(())
    }

    /// Waits up to `ms` for the process to exit.
    pub fn wait(&self, ms: u32) -> bool {
        unsafe { WaitForSingleObject(self.handle, ms) == WAIT_OBJECT_0 }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
