//! Starting STFD with Windows, for watching apps opened another way: a value under the
//! person's Run key (no administrator rights needed).

use super::{wide, SysResult};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};

const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const NAME: &str = "STFD";

/// The command Windows runs at sign-in, if set.
pub fn get() -> Option<String> {
    let key = wide(RUN);
    let name = wide(NAME);
    let mut buf = [0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size as *mut u32),
        )
    };
    r.is_ok().then(|| super::from_wide(&buf))
}

/// Starts `command` at sign-in.
pub fn set(command: &str) -> SysResult<()> {
    let key = wide(RUN);
    let name = wide(NAME);
    let data = wide(command);
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
        .ok()?;
    }
    Ok(())
}

/// Stops starting at sign-in; fine when it was not set.
pub fn remove() {
    let key = wide(RUN);
    let name = wide(NAME);
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), PCWSTR(name.as_ptr()));
    }
}
