//! Shortcut (.lnk) files: making Lockstep's, and finding the ones that open a program directly.

use super::{from_wide, SysError, SysResult};
use crate::paths;
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

/// Runs `f` with COM ready on this thread.
fn with_com<T>(f: impl FnOnce() -> SysResult<T>) -> SysResult<T> {
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // Already set up another way on this thread: use it as it is, and leave it.
    let ours = hr.is_ok();
    if !ours && hr != RPC_E_CHANGED_MODE {
        return Err(windows::core::Error::from(hr).into());
    }
    let result = f();
    if ours {
        unsafe { CoUninitialize() };
    }
    result
}

/// What a shortcut opens.
pub struct Link<'a> {
    pub target: &'a str,
    pub arguments: &'a str,
    pub working_dir: &'a str,
    pub icon: &'a str,
    pub description: &'a str,
}

/// Writes a shortcut file at `path` (replacing one that is there).
pub fn create(path: &str, link: &Link) -> SysResult<()> {
    with_com(|| unsafe {
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        shell.SetPath(&HSTRING::from(link.target))?;
        shell.SetArguments(&HSTRING::from(link.arguments))?;
        shell.SetWorkingDirectory(&HSTRING::from(link.working_dir))?;
        shell.SetIconLocation(&HSTRING::from(link.icon), 0)?;
        shell.SetDescription(&HSTRING::from(link.description))?;
        let file: IPersistFile = shell.cast()?;
        file.Save(&HSTRING::from(path), true)?;
        Ok(())
    })
}

/// The program a shortcut opens and its arguments; None when it is not a shortcut to a file.
pub fn read(path: &str) -> SysResult<(String, String)> {
    with_com(|| unsafe {
        let shell: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let file: IPersistFile = shell.cast()?;
        file.Load(&HSTRING::from(path), STGM_READ)?;
        let mut target = [0u16; 1024];
        shell.GetPath(&mut target, std::ptr::null_mut(), 0)?;
        let mut args = [0u16; 2048];
        shell.GetArguments(&mut args)?;
        let target = from_wide(&target);
        if target.is_empty() {
            return Err(SysError::Other("the shortcut does not open a file".into()));
        }
        Ok((target, from_wide(&args)))
    })
}

/// Shortcuts in `folders` (not their subfolders) that open `program` directly.
pub fn find_for(program: &str, folders: &[String]) -> Vec<String> {
    let program = super::long_path(program);
    let mut out = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
                continue;
            }
            let path = path.to_string_lossy().to_string();
            if let Ok((target, _)) = read(&path) {
                if paths::same(&super::long_path(&target), &program) {
                    out.push(path);
                }
            }
        }
    }
    out
}
