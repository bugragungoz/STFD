//! Reads and changes the real system on Windows: processes and their windows, services, shortcuts,
//! program details, the elevated step and starting with Windows. None of it needs administrator
//! rights except `elevated` and the service permission calls it makes.

pub mod autostart;
pub mod elevated;
pub mod execute;
pub mod folders;
pub mod icon;
pub mod launch;
pub mod process;
pub mod services;
pub mod shortcut;
pub mod version;

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, WIN32_ERROR};

#[derive(Debug, thiserror::Error)]
pub enum SysError {
    /// Windows refused: the thing belongs to an administrator or another account.
    #[error("access denied")]
    AccessDenied,
    #[error("{0}")]
    Windows(String),
    #[error("{0}")]
    Other(String),
}

impl From<windows::core::Error> for SysError {
    fn from(e: windows::core::Error) -> Self {
        if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
            SysError::AccessDenied
        } else {
            SysError::Windows(e.message().trim().to_string())
        }
    }
}

impl From<WIN32_ERROR> for SysError {
    fn from(e: WIN32_ERROR) -> Self {
        windows::core::Error::from(e.to_hresult()).into()
    }
}

pub type SysResult<T> = Result<T, SysError>;

/// The path with every short (8.3) folder or file name written out in full, the way processes and
/// shortcuts report it; the path as given when it does not exist.
pub fn long_path(path: &str) -> String {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetLongPathNameW;
    let w = wide(path);
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetLongPathNameW(PCWSTR(w.as_ptr()), Some(&mut buf)) } as usize;
    if n == 0 || n > buf.len() {
        return path.to_string();
    }
    String::from_utf16_lossy(&buf[..n])
}

/// A string as a null-terminated UTF-16 buffer.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A null-terminated UTF-16 string read into a String.
pub(crate) unsafe fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// The text of a buffer up to its first null.
pub(crate) fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}
