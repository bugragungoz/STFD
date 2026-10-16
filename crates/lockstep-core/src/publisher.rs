//! Publisher names from version resources, compared loosely: "ANSYS, Inc." and "Ansys Inc" are the
//! same publisher.

const SUFFIXES: &[&str] = &[
    "inc",
    "incorporated",
    "corp",
    "corporation",
    "co",
    "company",
    "ltd",
    "limited",
    "llc",
    "gmbh",
    "ag",
    "sa",
    "srl",
    "bv",
    "plc",
    "kg",
    "ab",
    "as",
    "oy",
];

/// Lowercase words without punctuation or a company-form suffix.
pub fn normalize(company: &str) -> String {
    let cleaned: String = company
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect();
    let mut words: Vec<&str> = cleaned.split_whitespace().collect();
    while words.len() > 1 && words.last().is_some_and(|w| SUFFIXES.contains(w)) {
        words.pop();
    }
    words.join(" ")
}

/// True when both name the same publisher.
pub fn same(a: &str, b: &str) -> bool {
    let a = normalize(a);
    !a.is_empty() && a == normalize(b)
}

/// Microsoft publishes Windows: "same publisher" would match half the system.
pub fn is_microsoft(company: &str) -> bool {
    normalize(company).starts_with("microsoft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn company_forms_and_punctuation_do_not_matter() {
        assert_eq!(normalize("ANSYS, Inc."), "ansys");
        assert!(same("ANSYS, Inc.", "Ansys Inc"));
        assert!(same("National Instruments Corporation", "National Instruments"));
        assert!(same("Valve Corp.", "Valve"));
        assert!(!same("NI", "National Instruments"));
        assert!(!same("", ""));
        assert_eq!(normalize("Co"), "co", "a single word stays");
    }

    #[test]
    fn microsoft_is_recognised() {
        assert!(is_microsoft("Microsoft Corporation"));
        assert!(is_microsoft("Microsoft Windows Publisher"));
        assert!(!is_microsoft("ANSYS, Inc."));
    }
}
