//! Windows paths compared the way Windows compares them: without regard to case, with either
//! slash, without the `\\?\` prefix or a trailing backslash. And the folders an app may not watch.

use serde::{Deserialize, Serialize};

/// The comparable form of a path: lowercase, backslashes, no `\\?\` prefix, no repeated or trailing
/// backslash (a drive keeps its own: `c:\`).
pub fn normalize(path: &str) -> String {
    let mut p = path.trim().replace('/', "\\");
    if let Some(rest) = p.strip_prefix("\\\\?\\UNC\\") {
        p = format!("\\\\{rest}");
    } else if let Some(rest) = p.strip_prefix("\\\\?\\") {
        p = rest.to_string();
    }
    let unc = p.starts_with("\\\\");
    let mut out = String::with_capacity(p.len());
    if unc {
        out.push_str("\\\\");
    }
    let mut last_was_slash = unc;
    for c in p.trim_start_matches('\\').chars() {
        if c == '\\' {
            if !last_was_slash {
                out.push('\\');
            }
            last_was_slash = true;
        } else {
            out.extend(c.to_lowercase());
            last_was_slash = false;
        }
    }
    while out.ends_with('\\') && !is_drive_root(&out) && out.len() > 2 {
        out.pop();
    }
    if out.len() == 2 && out.ends_with(':') {
        out.push('\\');
    }
    out
}

fn is_drive_root(normalized: &str) -> bool {
    let b = normalized.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

/// True when both name the same file or folder.
pub fn same(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

/// True when `path` is `folder` or lies anywhere under it.
pub fn is_inside(path: &str, folder: &str) -> bool {
    let p = normalize(path);
    let f = normalize(folder);
    if f.is_empty() {
        return false;
    }
    if f.ends_with('\\') {
        return p.starts_with(&f);
    }
    p == f || (p.starts_with(&f) && p.as_bytes().get(f.len()) == Some(&b'\\'))
}

/// The last part of a path: `multisim.exe` for `C:\NI\multisim.exe`.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// The folder a file is in, as written; None for a bare name.
pub fn parent(path: &str) -> Option<&str> {
    let trimmed = path.trim_end_matches(['\\', '/']);
    let cut = trimmed.rfind(['\\', '/'])?;
    let parent = &trimmed[..cut];
    // `C:` alone is the drive's current folder, not its root.
    if parent.len() == 2 && parent.ends_with(':') {
        Some(&trimmed[..cut + 1])
    } else {
        Some(parent)
    }
}

/// True for `C:\...` and `\\server\share\...`.
pub fn is_absolute(path: &str) -> bool {
    let p = path.trim().replace('/', "\\");
    let b = p.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\') || p.starts_with("\\\\")
}

/// A file name for a shortcut called `name`: characters Windows does not allow in file names become
/// spaces.
pub fn shortcut_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_end_matches('.').trim();
    format!("{}.lnk", if cleaned.is_empty() { "App" } else { cleaned })
}

/// The folders of this Windows that matter for what Lockstep may watch and close.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFolders {
    /// `C:\Windows`: nothing in it is ever closed or watched.
    pub windows: String,
    /// `C:\Program Files` and `C:\Program Files (x86)`.
    pub program_files: Vec<String>,
    /// `C:\Program Files\Common Files` and its x86 twin.
    pub common_files: Vec<String>,
    /// `C:\ProgramData`.
    pub program_data: String,
    /// `C:\Users`.
    pub users: String,
    /// The user's own folder, `C:\Users\name`.
    pub profile: String,
    /// `...\AppData\Roaming` and `...\AppData\Local`.
    pub app_data: Vec<String>,
    /// The user's desktop and the public one.
    pub desktops: Vec<String>,
    /// Lockstep's own install folder.
    pub own: String,
}

/// Why a folder cannot be an app's folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FolderProblem {
    /// Not a full path like `C:\Program Files\Vendor`.
    NotAbsolute,
    /// A whole drive.
    DriveRoot,
    /// Windows itself.
    System,
    /// A folder that holds many unrelated programs, such as Program Files itself.
    TooBroad,
    /// Lockstep's own folder.
    Own,
}

/// Checks a folder an app would watch. Everything under it counts as part of the app, so a folder
/// that holds unrelated programs would close them too.
pub fn check_folder(folder: &str, known: &KnownFolders) -> Result<(), FolderProblem> {
    let f = normalize(folder);
    if is_drive_root(&f) || (f.starts_with("\\\\") && f.trim_start_matches('\\').split('\\').count() <= 2) {
        return Err(FolderProblem::DriveRoot);
    }
    if !is_absolute(folder) {
        return Err(FolderProblem::NotAbsolute);
    }
    if !known.windows.is_empty() && (is_inside(&f, &known.windows) || is_inside(&known.windows, &f)) {
        return Err(FolderProblem::System);
    }
    let mut broad: Vec<&str> = Vec::new();
    broad.extend(known.program_files.iter().map(String::as_str));
    broad.extend(known.common_files.iter().map(String::as_str));
    broad.extend(known.app_data.iter().map(String::as_str));
    broad.extend(known.desktops.iter().map(String::as_str));
    broad.extend([
        known.program_data.as_str(),
        known.users.as_str(),
        known.profile.as_str(),
    ]);
    let app_data_parent = known.app_data.first().and_then(|a| parent(a));
    broad.extend(app_data_parent);
    for b in broad.into_iter().filter(|b| !b.is_empty()) {
        // The broad folder itself, or a folder above it (C:\Users holds every profile).
        if same(&f, b) || is_inside(b, &f) {
            return Err(FolderProblem::TooBroad);
        }
    }
    if !known.own.is_empty() && (is_inside(&f, &known.own) || is_inside(&known.own, &f)) {
        return Err(FolderProblem::Own);
    }
    Ok(())
}

/// True for a program that belongs to Windows itself and is never closed.
pub fn is_system(path: &str, known: &KnownFolders) -> bool {
    !known.windows.is_empty() && is_inside(path, &known.windows)
}

/// Folders of the same vendor elsewhere: for `C:\Program Files (x86)\National Instruments\Circuit
/// Design Suite 14.3`, the `National Instruments` folders under the other Program Files, Common Files
/// and ProgramData that `exists` finds. Many vendors keep their services there.
pub fn vendor_folders(folder: &str, known: &KnownFolders, exists: impl Fn(&str) -> bool) -> Vec<String> {
    let roots: Vec<&str> = known
        .program_files
        .iter()
        .chain(&known.common_files)
        .map(String::as_str)
        .chain([known.program_data.as_str()])
        .filter(|r| !r.is_empty())
        .collect();
    // The common-files roots lie inside program files; match the most specific root first.
    let mut by_length = roots.clone();
    by_length.sort_by_key(|r| std::cmp::Reverse(normalize(r).len()));
    let Some(root) = by_length.iter().find(|r| is_inside(folder, r) && !same(folder, r)) else {
        return Vec::new();
    };
    let rest = &normalize(folder)[normalize(root).len()..];
    let Some(vendor_lower) = rest
        .trim_start_matches('\\')
        .split('\\')
        .next()
        .filter(|v| !v.is_empty())
    else {
        return Vec::new();
    };
    // Keep the vendor folder's own spelling for display.
    let original = folder.trim().replace('/', "\\");
    let vendor = original
        .split('\\')
        .find(|part| part.to_lowercase() == vendor_lower)
        .unwrap_or(vendor_lower)
        .to_string();
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        let candidate = format!("{}\\{}", r.trim_end_matches('\\'), vendor);
        if is_inside(folder, &candidate) || out.iter().any(|o| same(o, &candidate)) {
            continue;
        }
        if exists(&candidate) {
            out.push(candidate);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> KnownFolders {
        KnownFolders {
            windows: "C:\\Windows".into(),
            program_files: vec!["C:\\Program Files".into(), "C:\\Program Files (x86)".into()],
            common_files: vec![
                "C:\\Program Files\\Common Files".into(),
                "C:\\Program Files (x86)\\Common Files".into(),
            ],
            program_data: "C:\\ProgramData".into(),
            users: "C:\\Users".into(),
            profile: "C:\\Users\\bugra".into(),
            app_data: vec![
                "C:\\Users\\bugra\\AppData\\Roaming".into(),
                "C:\\Users\\bugra\\AppData\\Local".into(),
            ],
            desktops: vec!["C:\\Users\\bugra\\Desktop".into(), "C:\\Users\\Public\\Desktop".into()],
            own: "C:\\Program Files\\Lockstep".into(),
        }
    }

    #[test]
    fn paths_compare_without_case_slashes_prefixes_or_trailing_backslashes() {
        assert_eq!(normalize("C:/Program Files//NI\\"), "c:\\program files\\ni");
        assert_eq!(normalize("\\\\?\\C:\\Tools\\a.exe"), "c:\\tools\\a.exe");
        assert_eq!(normalize("\\\\?\\UNC\\server\\share\\x"), "\\\\server\\share\\x");
        assert_eq!(normalize("c:"), "c:\\");
        assert_eq!(normalize("C:\\"), "c:\\");
        assert!(same("C:\\NI\\Multisim.EXE", "c:/ni/multisim.exe"));
    }

    #[test]
    fn inside_means_the_folder_or_under_it_not_a_name_that_starts_the_same() {
        assert!(is_inside("C:\\Program Files\\NI\\a.exe", "c:\\program files\\ni"));
        assert!(is_inside("C:\\Program Files\\NI", "C:\\Program Files\\NI\\"));
        assert!(!is_inside(
            "C:\\Program Files\\NI Tools\\a.exe",
            "C:\\Program Files\\NI"
        ));
        assert!(is_inside("D:\\Games\\x.exe", "D:\\"));
        assert!(!is_inside("C:\\a.exe", ""));
    }

    #[test]
    fn names_and_parents() {
        assert_eq!(file_name("C:\\NI\\multisim.exe"), "multisim.exe");
        assert_eq!(file_name("multisim.exe"), "multisim.exe");
        assert_eq!(parent("C:\\NI\\multisim.exe"), Some("C:\\NI"));
        assert_eq!(parent("C:\\NI"), Some("C:\\"));
        assert_eq!(parent("multisim.exe"), None);
        assert!(is_absolute("C:\\x") && is_absolute("\\\\srv\\share") && !is_absolute("x\\y") && !is_absolute("C:x"));
    }

    #[test]
    fn shortcut_names_are_valid_file_names() {
        assert_eq!(shortcut_file_name("Multisim 14.3"), "Multisim 14.3.lnk");
        assert_eq!(shortcut_file_name("A/B: C?"), "A B C.lnk");
        assert_eq!(shortcut_file_name("Name..."), "Name.lnk");
        assert_eq!(shortcut_file_name(" \t "), "App.lnk");
    }

    #[test]
    fn a_vendor_folder_can_be_watched() {
        let k = known();
        assert_eq!(
            check_folder("C:\\Program Files (x86)\\National Instruments", &k),
            Ok(())
        );
        assert_eq!(check_folder("C:\\Program Files\\AnsysEM\\v241", &k), Ok(()));
        assert_eq!(check_folder("D:\\Games\\Some Game", &k), Ok(()));
        assert_eq!(check_folder("C:\\Users\\bugra\\AppData\\Local\\Discord", &k), Ok(()));
    }

    #[test]
    fn folders_that_hold_unrelated_programs_or_windows_cannot() {
        let k = known();
        assert_eq!(check_folder("relative\\path", &k), Err(FolderProblem::NotAbsolute));
        assert_eq!(check_folder("C:\\", &k), Err(FolderProblem::DriveRoot));
        assert_eq!(check_folder("D:", &k), Err(FolderProblem::DriveRoot));
        assert_eq!(check_folder("\\\\server\\share", &k), Err(FolderProblem::DriveRoot));
        assert_eq!(check_folder("C:\\Windows\\System32", &k), Err(FolderProblem::System));
        assert_eq!(check_folder("C:\\Program Files", &k), Err(FolderProblem::TooBroad));
        assert_eq!(
            check_folder("c:\\program files (x86)\\", &k),
            Err(FolderProblem::TooBroad)
        );
        assert_eq!(
            check_folder("C:\\Program Files\\Common Files", &k),
            Err(FolderProblem::TooBroad)
        );
        assert_eq!(check_folder("C:\\Users", &k), Err(FolderProblem::TooBroad));
        assert_eq!(
            check_folder("C:\\Users\\bugra\\AppData", &k),
            Err(FolderProblem::TooBroad)
        );
        assert_eq!(
            check_folder("C:\\Users\\bugra\\Desktop", &k),
            Err(FolderProblem::TooBroad)
        );
        assert_eq!(check_folder("C:\\Program Files\\Lockstep", &k), Err(FolderProblem::Own));
    }

    #[test]
    fn windows_programs_are_system() {
        let k = known();
        assert!(is_system("C:\\WINDOWS\\explorer.exe", &k));
        assert!(!is_system("C:\\Program Files\\NI\\x.exe", &k));
        assert!(!is_system("C:\\x.exe", &KnownFolders::default()));
    }

    #[test]
    fn the_same_vendor_is_found_under_the_other_roots() {
        let k = known();
        let exists = |p: &str| {
            [
                "C:\\Program Files\\National Instruments",
                "C:\\Program Files (x86)\\Common Files\\National Instruments",
                "C:\\ProgramData\\National Instruments",
            ]
            .iter()
            .any(|e| same(e, p))
        };
        let found = vendor_folders(
            "C:\\Program Files (x86)\\National Instruments\\Circuit Design Suite 14.3",
            &k,
            exists,
        );
        assert_eq!(
            found,
            vec![
                "C:\\Program Files\\National Instruments",
                "C:\\Program Files (x86)\\Common Files\\National Instruments",
                "C:\\ProgramData\\National Instruments",
            ]
        );
        assert!(vendor_folders("D:\\Games\\X", &k, |_| true).is_empty());
        assert!(vendor_folders("C:\\Program Files", &k, |_| true).is_empty());
    }
}
