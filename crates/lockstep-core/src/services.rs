//! Services that belong to an app: the ones whose program lives in the app's folders, and the ones
//! from the same publisher elsewhere (license servers often sit apart from the program).

use crate::model::StartType;
use crate::paths::{self, KnownFolders};
use crate::publisher;
use crate::snapshot::SvcState;
use serde::Serialize;

/// A service as the Windows side lists it for choosing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub display: String,
    pub state: SvcState,
    pub start_type: StartType,
    /// The program it runs, from its command line.
    pub program: Option<String>,
    /// For services in a shared host (svchost.exe), the DLL that is the service.
    pub dll: Option<String>,
    /// The publisher of its program or DLL.
    pub company: Option<String>,
    /// True when this person may start and stop it without administrator rights.
    pub controllable: bool,
}

/// Why a service is suggested for an app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    InFolder,
    SamePublisher,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggested {
    #[serde(flatten)]
    pub service: ServiceInfo,
    pub why: Why,
}

/// The services that look like part of an app: in its folders first, then from the same publisher
/// (never Microsoft's, never Windows' own), each sorted by display name.
pub fn related(
    services: &[ServiceInfo],
    folders: &[String],
    company: Option<&str>,
    known: &KnownFolders,
) -> Vec<Suggested> {
    let in_folders = |p: &Option<String>| {
        p.as_deref()
            .is_some_and(|p| folders.iter().any(|f| paths::is_inside(p, f)))
    };
    let publisher = company.filter(|c| !c.trim().is_empty() && !publisher::is_microsoft(c));
    let mut out: Vec<Suggested> = Vec::new();
    for s in services {
        let why = if in_folders(&s.program) || in_folders(&s.dll) {
            Why::InFolder
        } else if publisher.is_some_and(|p| s.company.as_deref().is_some_and(|c| publisher::same(c, p)))
            && !s.program.as_deref().is_some_and(|p| paths::is_system(p, known))
        {
            Why::SamePublisher
        } else {
            continue;
        };
        out.push(Suggested {
            service: s.clone(),
            why,
        });
    }
    out.sort_by(|a, b| {
        (a.why == Why::SamePublisher)
            .cmp(&(b.why == Why::SamePublisher))
            .then_with(|| a.service.display.to_lowercase().cmp(&b.service.display.to_lowercase()))
    });
    out
}

/// The program a service runs, from its command line (ImagePath): quoted, unquoted with spaces
/// (`exists` tells which prefix is the file), with `\??\`, `\SystemRoot\` or `%SystemRoot%`, or a
/// path relative to the Windows folder (drivers).
pub fn program_of(image_path: &str, system_root: &str, exists: impl Fn(&str) -> bool) -> Option<String> {
    let text = image_path.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(rest) = text.strip_prefix('"') {
        let end = rest.find('"').unwrap_or(rest.len());
        return Some(expand(&rest[..end], system_root));
    }
    let expanded = expand(text, system_root);
    let words: Vec<&str> = expanded.split(' ').collect();
    let mut first_exe: Option<String> = None;
    for n in 1..=words.len() {
        let candidate = words[..n].join(" ");
        if candidate.to_ascii_lowercase().ends_with(".exe") {
            if exists(&candidate) {
                return Some(candidate);
            }
            first_exe.get_or_insert(candidate);
        }
    }
    Some(first_exe.unwrap_or_else(|| words[0].to_string()))
}

fn expand(path: &str, system_root: &str) -> String {
    let mut p = path.trim().to_string();
    if let Some(rest) = p.strip_prefix("\\??\\") {
        p = rest.to_string();
    }
    let lower = p.to_ascii_lowercase();
    for var in ["%systemroot%", "%windir%"] {
        if lower.starts_with(var) {
            return format!("{}{}", system_root, &p[var.len()..]);
        }
    }
    if lower.starts_with("\\systemroot\\") {
        return format!("{}{}", system_root, &p["\\systemroot".len()..]);
    }
    if lower.starts_with("system32\\") || lower.starts_with("syswow64\\") {
        return format!("{}\\{}", system_root, p);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: &str = "C:\\Windows";

    #[test]
    fn quoted_unquoted_and_system_paths() {
        let exists = |p: &str| p == "C:\\Program Files\\ANSYS Inc\\Shared Files\\Licensing\\winx64\\lmgrd.exe";
        assert_eq!(
            program_of("\"C:\\Program Files\\NI\\nisvc.exe\" -service", WIN, |_| false).as_deref(),
            Some("C:\\Program Files\\NI\\nisvc.exe")
        );
        assert_eq!(
            program_of(
                "C:\\Program Files\\ANSYS Inc\\Shared Files\\Licensing\\winx64\\lmgrd.exe -c x.lic -l log.txt",
                WIN,
                exists
            )
            .as_deref(),
            Some("C:\\Program Files\\ANSYS Inc\\Shared Files\\Licensing\\winx64\\lmgrd.exe")
        );
        assert_eq!(
            program_of("C:\\Program Files\\My App\\svc.exe -x", WIN, |_| false).as_deref(),
            Some("C:\\Program Files\\My App\\svc.exe"),
            "the first .exe when none can be checked"
        );
        assert_eq!(
            program_of("%SystemRoot%\\System32\\svchost.exe -k netsvcs -p", WIN, |_| false).as_deref(),
            Some("C:\\Windows\\System32\\svchost.exe")
        );
        assert_eq!(
            program_of("\\SystemRoot\\System32\\drivers\\x.sys", WIN, |_| false).as_deref(),
            Some("C:\\Windows\\System32\\drivers\\x.sys")
        );
        assert_eq!(
            program_of("\\??\\C:\\Tools\\svc.exe", WIN, |_| false).as_deref(),
            Some("C:\\Tools\\svc.exe")
        );
        assert_eq!(
            program_of("system32\\DRIVERS\\y.sys", WIN, |_| false).as_deref(),
            Some("C:\\Windows\\system32\\DRIVERS\\y.sys")
        );
        assert_eq!(program_of("  ", WIN, |_| false), None);
    }

    fn info(name: &str, program: Option<&str>, dll: Option<&str>, company: Option<&str>) -> ServiceInfo {
        ServiceInfo {
            name: name.into(),
            display: name.into(),
            state: SvcState::Stopped,
            start_type: StartType::Manual,
            program: program.map(Into::into),
            dll: dll.map(Into::into),
            company: company.map(Into::into),
            controllable: false,
        }
    }

    #[test]
    fn services_in_the_folders_come_first_then_the_same_publisher() {
        let known = KnownFolders {
            windows: WIN.into(),
            ..Default::default()
        };
        let services = vec![
            info(
                "Zeta",
                Some("C:\\Program Files\\AnsysEM\\v241\\zeta.exe"),
                None,
                Some("ANSYS, Inc."),
            ),
            info(
                "Licensing",
                Some("C:\\Program Files\\ANSYS Inc\\Shared Files\\lmgrd.exe"),
                None,
                Some("ANSYS Inc"),
            ),
            info(
                "Hosted",
                Some("C:\\Windows\\System32\\svchost.exe"),
                Some("C:\\Program Files\\AnsysEM\\x.dll"),
                None,
            ),
            info(
                "Spooler",
                Some("C:\\Windows\\System32\\spoolsv.exe"),
                None,
                Some("Microsoft Corporation"),
            ),
            info("WinAnsys", Some("C:\\Windows\\ansys.exe"), None, Some("ANSYS, Inc.")),
            info("Other", Some("C:\\Program Files\\Other\\o.exe"), None, Some("Other Co")),
        ];
        let found = related(
            &services,
            &["C:\\Program Files\\AnsysEM".into()],
            Some("ANSYS, Inc."),
            &known,
        );
        let names: Vec<_> = found.iter().map(|s| (s.service.name.as_str(), s.why)).collect();
        assert_eq!(
            names,
            vec![
                ("Hosted", Why::InFolder),
                ("Zeta", Why::InFolder),
                ("Licensing", Why::SamePublisher),
            ]
        );
    }

    #[test]
    fn microsoft_programs_suggest_no_services_by_publisher() {
        let known = KnownFolders::default();
        let services = vec![info(
            "Spooler",
            Some("C:\\Windows\\System32\\spoolsv.exe"),
            None,
            Some("Microsoft Corporation"),
        )];
        assert!(related(
            &services,
            &["C:\\Program Files\\Office".into()],
            Some("Microsoft Corporation"),
            &known
        )
        .is_empty());
        assert!(related(&services, &["C:\\Program Files\\Office".into()], None, &known).is_empty());
    }
}
