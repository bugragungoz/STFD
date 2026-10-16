//! Where this Windows keeps things: the desktop, the Start menu, Program Files and the rest.

use super::from_wide_ptr;
use crate::paths::KnownFolders;
use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_LocalAppData, FOLDERID_Profile, FOLDERID_ProgramData, FOLDERID_ProgramFiles,
    FOLDERID_ProgramFilesCommon, FOLDERID_ProgramFilesCommonX86, FOLDERID_ProgramFilesX86, FOLDERID_Programs,
    FOLDERID_PublicDesktop, FOLDERID_RoamingAppData, FOLDERID_UserProfiles, FOLDERID_Windows, SHGetKnownFolderPath,
    KNOWN_FOLDER_FLAG,
};

fn known(id: &GUID) -> Option<String> {
    unsafe {
        let p = SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None).ok()?;
        let s = from_wide_ptr(p.0);
        CoTaskMemFree(Some(p.0 as *const _));
        (!s.is_empty()).then_some(s)
    }
}

/// The person's desktop (wherever OneDrive moved it).
pub fn desktop() -> Option<String> {
    known(&FOLDERID_Desktop)
}

/// The desktop every account shares; installers often put shortcuts here.
pub fn public_desktop() -> Option<String> {
    known(&FOLDERID_PublicDesktop)
}

/// The person's Start menu programs folder.
pub fn start_menu() -> Option<String> {
    known(&FOLDERID_Programs)
}

/// The folders Lockstep must know about. `own` is Lockstep's install folder.
pub fn all(own: &str) -> KnownFolders {
    let some = |id: &GUID| known(id).unwrap_or_default();
    KnownFolders {
        windows: some(&FOLDERID_Windows),
        program_files: [known(&FOLDERID_ProgramFiles), known(&FOLDERID_ProgramFilesX86)]
            .into_iter()
            .flatten()
            .collect(),
        common_files: [
            known(&FOLDERID_ProgramFilesCommon),
            known(&FOLDERID_ProgramFilesCommonX86),
        ]
        .into_iter()
        .flatten()
        .collect(),
        program_data: some(&FOLDERID_ProgramData),
        users: some(&FOLDERID_UserProfiles),
        profile: some(&FOLDERID_Profile),
        app_data: [known(&FOLDERID_RoamingAppData), known(&FOLDERID_LocalAppData)]
            .into_iter()
            .flatten()
            .collect(),
        desktops: [desktop(), public_desktop()].into_iter().flatten().collect(),
        own: own.to_string(),
    }
}
