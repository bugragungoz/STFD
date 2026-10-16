//! The few words STFD shows outside its window (notifications and the tray), in English or
//! Turkish like the window.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Tr,
}

/// The language the settings ask for; "system" follows Windows.
pub fn pick(setting: &str) -> Lang {
    match setting {
        "tr" => Lang::Tr,
        "en" => Lang::En,
        _ => system(),
    }
}

#[cfg(windows)]
fn system() -> Lang {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    let name = String::from_utf16_lossy(&buf[..(n.max(1) as usize - 1)]);
    if name.to_lowercase().starts_with("tr") {
        Lang::Tr
    } else {
        Lang::En
    }
}

#[cfg(not(windows))]
fn system() -> Lang {
    let l = std::env::var("LANG").unwrap_or_default();
    if l.starts_with("tr") {
        Lang::Tr
    } else {
        Lang::En
    }
}

impl Lang {
    pub fn open(self) -> &'static str {
        match self {
            Lang::En => "Open STFD",
            Lang::Tr => "STFD'yi aç",
        }
    }

    pub fn quit(self) -> &'static str {
        match self {
            Lang::En => "Quit",
            Lang::Tr => "Çık",
        }
    }

    /// The tray's tooltip: what STFD follows now.
    pub fn tooltip(self, open: &[String], closing: &[(String, u64)]) -> String {
        let mut lines = vec!["STFD".to_string()];
        for (name, seconds) in closing {
            lines.push(match self {
                Lang::En => format!("{name}: closing the rest in {seconds} s"),
                Lang::Tr => format!("{name}: kalanlar {seconds} sn sonra kapanacak"),
            });
        }
        if !open.is_empty() {
            lines.push(match self {
                Lang::En => format!("Following {}", open.join(", ")),
                Lang::Tr => format!("İzleniyor: {}", open.join(", ")),
            });
        }
        if lines.len() == 1 {
            lines.push(match self {
                Lang::En => "Watching your apps".into(),
                Lang::Tr => "Uygulamalarınız izleniyor".into(),
            });
        }
        // Windows cuts tray tooltips at 127 characters.
        let text = lines.join("\n");
        text.chars().take(120).collect()
    }

    /// After a cleanup: "Stopped 7 services and closed 2 programs."
    pub fn cleaned(self, app: &str, services: usize, programs: usize, problems: usize) -> (String, String) {
        let title = match self {
            Lang::En => format!("{app} closed"),
            Lang::Tr => format!("{app} kapandı"),
        };
        let mut parts = Vec::new();
        match self {
            Lang::En => {
                if services > 0 {
                    parts.push(format!(
                        "stopped {} {}",
                        services,
                        if services == 1 { "service" } else { "services" }
                    ));
                }
                if programs > 0 {
                    parts.push(format!(
                        "closed {} {}",
                        programs,
                        if programs == 1 { "program" } else { "programs" }
                    ));
                }
            }
            Lang::Tr => {
                if services > 0 {
                    parts.push(format!("{services} hizmet durduruldu"));
                }
                if programs > 0 {
                    parts.push(format!("{programs} program kapatıldı"));
                }
            }
        }
        let mut body = match (self, parts.len()) {
            (_, 0) => String::new(),
            (Lang::En, _) => {
                let joined = parts.join(" and ");
                let mut c = joined.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
                    + "."
            }
            (Lang::Tr, _) => {
                let joined = parts.join(", ");
                let mut c = joined.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
                    + "."
            }
        };
        if problems > 0 {
            if !body.is_empty() {
                body.push(' ');
            }
            body.push_str(&match self {
                Lang::En => format!("{problems} could not be closed; see History."),
                Lang::Tr => format!("{problems} tanesi kapatılamadı; Geçmiş'e bakın."),
            });
        }
        (title, body)
    }

    pub fn not_opened(self, app: &str) -> (String, String) {
        match self {
            Lang::En => (
                format!("{app} did not open"),
                "STFD stopped what it had started for it.".into(),
            ),
            Lang::Tr => (
                format!("{app} açılmadı"),
                "STFD onun için başlattıklarını durdurdu.".into(),
            ),
        }
    }

    pub fn service_not_started(self, app: &str, services: &[String]) -> (String, String) {
        let list = services.join(", ");
        match self {
            Lang::En => (
                format!("{app}: a service did not start"),
                format!("{list}. {app} opens anyway."),
            ),
            Lang::Tr => (
                format!("{app}: bir hizmet başlamadı"),
                format!("{list}. {app} yine de açılıyor."),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_notices_say_what_happened() {
        assert_eq!(
            Lang::En.cleaned("Circuit Simulator", 7, 1, 0),
            (
                "Circuit Simulator closed".to_string(),
                "Stopped 7 services and closed 1 program.".to_string()
            )
        );
        assert_eq!(
            Lang::En.cleaned("A", 0, 2, 1).1,
            "Closed 2 programs. 1 could not be closed; see History."
        );
        assert_eq!(
            Lang::Tr.cleaned("Circuit Simulator", 7, 2, 0),
            (
                "Circuit Simulator kapandı".to_string(),
                "7 hizmet durduruldu, 2 program kapatıldı.".to_string()
            )
        );
        assert_eq!(
            Lang::Tr.cleaned("A", 0, 0, 2).1,
            "2 tanesi kapatılamadı; Geçmiş'e bakın."
        );
    }

    #[test]
    fn the_tooltip_lists_what_is_followed_and_stays_short() {
        assert_eq!(Lang::En.tooltip(&[], &[]), "STFD\nWatching your apps");
        assert_eq!(
            Lang::En.tooltip(&["Circuit Simulator".into()], &[("Field Solver".into(), 25)]),
            "STFD\nField Solver: closing the rest in 25 s\nFollowing Circuit Simulator"
        );
        let many: Vec<String> = (0..40).map(|i| format!("App number {i}")).collect();
        assert!(Lang::Tr.tooltip(&many, &[]).chars().count() <= 120);
    }

    #[test]
    fn languages_are_chosen_by_the_setting() {
        assert!(pick("tr") == Lang::Tr);
        assert!(pick("en") == Lang::En);
    }
}
