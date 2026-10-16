//! The programs in a folder, the likely main one first, for choosing what the shortcut opens.

use crate::paths;
use serde::Serialize;

/// A program found in the folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeFile {
    pub path: String,
    pub size: u64,
    /// Folders below the chosen one (0 = directly in it).
    pub depth: u32,
    /// True for a program with windows, false for a console one, None when unknown.
    pub gui: Option<bool>,
    /// The program's own description, or its product name.
    pub description: Option<String>,
    /// Uninstallers, updaters, crash reporters and the like.
    pub noise: bool,
}

const NOISE_START: &[&str] = &[
    "unins",
    "uninst",
    "setup",
    "install",
    "update",
    "updater",
    "crashpad",
    "crashreport",
    "crash_",
    "vc_redist",
    "vcredist",
    "dotnet",
    "dxsetup",
    "elevat",
    "repair",
    "bugreport",
    "regasm",
    "squirrel",
    "createdump",
    "7z",
];
const NOISE_ANYWHERE: &[&str] = &[
    "uninstall",
    "crash",
    "redist",
    "helper",
    "update",
    "notification",
    "telemetry",
];

/// True for programs nobody opens on purpose: uninstallers, updaters, crash reporters, helpers.
pub fn is_noise(file_name: &str) -> bool {
    let n = file_name.to_lowercase();
    NOISE_START.iter().any(|p| n.starts_with(p)) || NOISE_ANYWHERE.iter().any(|p| n.contains(p))
}

/// Orders programs the likely main one first: real programs before noise, windows before consoles,
/// shallow before deep, a name like the folder's, then larger files.
pub fn rank(files: &mut [ExeFile], folder: &str) {
    let folder_words: Vec<String> = words(paths::file_name(folder.trim_end_matches(['\\', '/'])));
    let score = |f: &ExeFile| -> i64 {
        let mut s = 0i64;
        if f.noise {
            s -= 1000;
        }
        match f.gui {
            Some(true) => s += 300,
            Some(false) => s -= 200,
            None => {}
        }
        s -= i64::from(f.depth.min(10)) * 40;
        let stem = paths::file_name(&f.path)
            .trim_end_matches(".exe")
            .trim_end_matches(".EXE");
        let name_words = words(stem);
        if name_words.iter().any(|w| {
            w.len() >= 3
                && folder_words
                    .iter()
                    .any(|fw| fw.starts_with(w.as_str()) || w.starts_with(fw.as_str()))
        }) {
            s += 150;
        }
        if f.description.is_some() {
            s += 30;
        }
        // Up to 100 more for size, by its order of magnitude (1 MB = 60, 100 MB = 80).
        s += ((f.size.max(1) as f64).log10() * 10.0).min(100.0) as i64;
        s
    };
    files.sort_by(|a, b| {
        score(b)
            .cmp(&score(a))
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
}

/// Lists the programs under `folder` (at most `limit`, at most 6 folders deep, not following links),
/// reads whether each has windows, ranks them, and asks `describe` for the description of the first
/// `described` ones.
pub fn scan(folder: &str, limit: usize, described: usize, describe: &dyn Fn(&str) -> Option<String>) -> Vec<ExeFile> {
    use std::io::Read;
    let mut files: Vec<ExeFile> = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, u32)> = vec![(std::path::PathBuf::from(folder), 0)];
    let mut visited = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            visited += 1;
            if files.len() >= limit || visited > 200_000 {
                break;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() {
                if depth < 6 {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !kind.is_file() || !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
                continue;
            }
            let mut head = [0u8; 4096];
            let read = std::fs::File::open(&path)
                .and_then(|mut f| f.read(&mut head))
                .unwrap_or(0);
            let gui = crate::pe::subsystem(&head[..read]).map(|s| s == crate::pe::Subsystem::Gui);
            let text = path.to_string_lossy().to_string();
            files.push(ExeFile {
                noise: is_noise(paths::file_name(&text)),
                size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                depth,
                gui,
                description: None,
                path: text,
            });
        }
    }
    rank(&mut files, folder);
    for f in files.iter_mut().take(described) {
        f.description = describe(&f.path);
    }
    files
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(path: &str, size: u64, depth: u32, gui: Option<bool>) -> ExeFile {
        ExeFile {
            path: path.into(),
            size,
            depth,
            gui,
            description: None,
            noise: is_noise(paths::file_name(path)),
        }
    }

    #[test]
    fn noise_is_recognised() {
        for n in [
            "unins000.exe",
            "Uninstall.exe",
            "setup.exe",
            "ELUpdateService.exe",
            "crashpad_handler.exe",
            "vc_redist.x64.exe",
            "Chat Helper.exe",
        ] {
            assert!(is_noise(n), "{n}");
        }
        for n in ["circuitsim.exe", "fieldsolver.exe", "game.exe"] {
            assert!(!is_noise(n), "{n}");
        }
    }

    #[test]
    fn the_main_program_comes_first() {
        let folder = "C:\\Program Files (x86)\\Example Labs\\Circuit Simulator 14";
        let mut files = vec![
            exe(&format!("{folder}\\unins000.exe"), 3_000_000, 0, Some(true)),
            exe(&format!("{folder}\\tools\\convert.exe"), 400_000, 1, Some(false)),
            exe(&format!("{folder}\\boarddesigner.exe"), 30_000_000, 0, Some(true)),
            exe(&format!("{folder}\\circuitsim.exe"), 60_000_000, 0, Some(true)),
            exe(&format!("{folder}\\deep\\a\\b\\viewer.exe"), 90_000_000, 3, Some(true)),
        ];
        rank(&mut files, folder);
        let names: Vec<_> = files.iter().map(|f| paths::file_name(&f.path)).collect();
        assert_eq!(
            names,
            vec![
                "circuitsim.exe",
                "boarddesigner.exe",
                "viewer.exe",
                "convert.exe",
                "unins000.exe"
            ]
        );
    }

    #[test]
    fn a_name_like_the_folders_wins_among_equals() {
        let folder = "D:\\Games\\Sample Game";
        let mut files = vec![
            exe(&format!("{folder}\\launcher.exe"), 5_000_000, 0, Some(true)),
            exe(&format!("{folder}\\sample_game.exe"), 5_000_000, 0, Some(true)),
        ];
        rank(&mut files, folder);
        assert_eq!(paths::file_name(&files[0].path), "sample_game.exe");
    }

    #[test]
    fn scanning_a_folder_finds_its_programs() {
        let dir = std::env::temp_dir().join(format!("stfd-scan-{}", crate::new_id()));
        std::fs::create_dir_all(dir.join("bin").join("deep")).unwrap();
        let mut gui = vec![0u8; 512];
        gui[0..2].copy_from_slice(b"MZ");
        gui[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        gui[0x80..0x84].copy_from_slice(b"PE\0\0");
        gui[0x80 + 24..0x80 + 26].copy_from_slice(&0x20bu16.to_le_bytes());
        gui[0x80 + 24 + 68..0x80 + 24 + 70].copy_from_slice(&2u16.to_le_bytes());
        std::fs::write(dir.join("bin").join("app.exe"), &gui).unwrap();
        std::fs::write(dir.join("unins000.exe"), &gui).unwrap();
        std::fs::write(dir.join("bin").join("deep").join("tool.EXE"), b"not a program").unwrap();
        std::fs::write(dir.join("readme.txt"), b"x").unwrap();

        let found = scan(&dir.to_string_lossy(), 100, 1, &|p| {
            Some(format!("about {}", paths::file_name(p)))
        });
        let names: Vec<_> = found.iter().map(|f| paths::file_name(&f.path)).collect();
        assert_eq!(names, vec!["app.exe", "tool.EXE", "unins000.exe"]);
        assert_eq!(found[0].gui, Some(true));
        assert_eq!(found[0].depth, 1);
        assert_eq!(found[0].description.as_deref(), Some("about app.exe"));
        assert_eq!(found[1].gui, None);
        assert_eq!(found[1].description, None, "only the first is described");
        assert!(found[2].noise);
        assert_eq!(
            scan(&dir.to_string_lossy(), 1, 0, &|_| None).len(),
            1,
            "the limit holds"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
