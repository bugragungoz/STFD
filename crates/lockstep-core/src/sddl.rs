//! Letting one person start and stop one service without administrator rights.
//!
//! A service's permissions are a security descriptor, written as SDDL:
//! `D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;...;;;BA)S:(AU;FA;...;;;WD)`. Lockstep adds one entry to the
//! `D:` part (the DACL) that allows the person's account (their SID) to query, start and stop the
//! service, and nothing else. Setting it needs administrator rights once; afterwards Lockstep starts
//! and stops the service as the person. The original text is kept to put back.

/// Query configuration (CC), query status (LC), start (RP), stop (WP), interrogate (LO) and read
/// the permissions (RC).
pub const RIGHTS: &str = "CCLCRPWPLORC";

/// The entry Lockstep adds for `sid`.
pub fn entry(sid: &str) -> String {
    format!("(A;;{RIGHTS};;;{sid})")
}

/// The ways Windows may write the entry back: with the SID, or for the computer's built-in
/// Administrator (RID 500) and Guest (501) with their abbreviations `LA` and `LG`.
fn entries(sid: &str) -> Vec<String> {
    let mut out = vec![entry(sid).to_ascii_uppercase()];
    if sid.ends_with("-500") {
        out.push(entry("LA"));
    } else if sid.ends_with("-501") {
        out.push(entry("LG"));
    }
    out
}

/// The top-level parts of an SDDL string: (`O`, `G`, `D` or `S`, start, end), where start is just
/// after the `X:` and end is where the next part begins.
fn parts(sddl: &str) -> Vec<(u8, usize, usize)> {
    let b = sddl.as_bytes();
    let mut depth = 0i32;
    let mut starts: Vec<(u8, usize)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            c @ (b'O' | b'G' | b'D' | b'S') if depth == 0 && b.get(i + 1) == Some(&b':') => {
                starts.push((c, i + 2));
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    let mut out = Vec::new();
    for (k, (tag, start)) in starts.iter().enumerate() {
        let end = starts.get(k + 1).map(|(_, s)| s - 2).unwrap_or(b.len());
        out.push((*tag, *start, end));
    }
    out
}

/// True when the permissions already let `sid` start and stop the service the way Lockstep would.
pub fn has_entry(sddl: &str, sid: &str) -> bool {
    let wanted = entries(sid);
    parts(sddl)
        .into_iter()
        .filter(|(tag, ..)| *tag == b'D')
        .any(|(_, start, end)| {
            let dacl = sddl[start..end].to_ascii_uppercase();
            wanted.iter().any(|w| dacl.contains(w))
        })
}

/// The permissions with Lockstep's entry for `sid` added at the end of the DACL. None when nothing
/// needs to change: the entry is there already, or there is no DACL to add to (no `D:` part, or
/// `NO_ACCESS_CONTROL`, which already allows everyone everything).
pub fn with_entry(sddl: &str, sid: &str) -> Option<String> {
    if !is_account_sid(sid) || has_entry(sddl, sid) {
        return None;
    }
    let (_, start, end) = parts(sddl).into_iter().find(|(tag, ..)| *tag == b'D')?;
    if sddl[start..end].contains("NO_ACCESS_CONTROL") {
        return None;
    }
    // Allow entries go after deny entries; the end of the DACL keeps that order.
    Some(format!("{}{}{}", &sddl[..end], entry(sid), &sddl[end..]))
}

/// The permissions without Lockstep's entry for `sid`; None when it is not there.
pub fn without_entry(sddl: &str, sid: &str) -> Option<String> {
    let (_, start, end) = parts(sddl).into_iter().find(|(tag, ..)| *tag == b'D')?;
    let dacl = sddl[start..end].to_ascii_uppercase();
    let (at, len) = entries(sid)
        .iter()
        .find_map(|w| dacl.find(w.as_str()).map(|i| (i + start, w.len())))?;
    Some(format!("{}{}", &sddl[..at], &sddl[at + len..]))
}

/// True for the SID of a person's account (`S-1-5-21-...`), not a group like Everyone or Users.
pub fn is_account_sid(sid: &str) -> bool {
    let Some(rest) = sid.strip_prefix("S-1-5-21-") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('-').collect();
    parts.len() >= 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "S-1-5-21-1004336348-1177238915-682003330-1001";
    // A typical third-party service, as `sc sdshow` prints it.
    const TYPICAL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)";

    #[test]
    fn the_entry_goes_at_the_end_of_the_dacl_before_the_audit_part() {
        let new = with_entry(TYPICAL, SID).unwrap();
        assert_eq!(
            new,
            format!(
                "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)(A;;CCLCRPWPLORC;;;{SID})S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)"
            )
        );
        assert!(has_entry(&new, SID));
        assert_eq!(with_entry(&new, SID), None, "added once");
    }

    #[test]
    fn owner_group_flags_and_a_missing_audit_part_are_kept() {
        let sddl = "O:SYG:SYD:P(D;;WP;;;WD)(A;;CCLCSWRPWPDTLOCRRC;;;SY)";
        assert_eq!(
            with_entry(sddl, SID).unwrap(),
            format!("O:SYG:SYD:P(D;;WP;;;WD)(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCLCRPWPLORC;;;{SID})")
        );
    }

    #[test]
    fn sids_inside_entries_are_not_taken_for_parts() {
        let sddl = "O:S-1-5-18D:(A;;RP;;;S-1-5-32-544)S:NO_ACCESS_CONTROL";
        let new = with_entry(sddl, SID).unwrap();
        assert_eq!(
            new,
            format!("O:S-1-5-18D:(A;;RP;;;S-1-5-32-544)(A;;CCLCRPWPLORC;;;{SID})S:NO_ACCESS_CONTROL")
        );
    }

    #[test]
    fn nothing_to_add_without_a_dacl_or_a_real_sid() {
        assert_eq!(with_entry("O:SYG:SY", SID), None);
        assert_eq!(with_entry("D:NO_ACCESS_CONTROL", SID), None);
        assert_eq!(with_entry(TYPICAL, ""), None);
        assert_eq!(with_entry(TYPICAL, "BU"), None, "only a person's SID");
        assert!(!has_entry(TYPICAL, SID));
    }

    #[test]
    fn the_check_ignores_the_case_of_the_entry() {
        let sddl = format!(
            "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(a;;cclcrpwplorc;;;{})",
            SID.to_lowercase()
        );
        assert!(has_entry(&sddl, SID));
        assert_eq!(with_entry(&sddl, SID), None);
    }

    #[test]
    fn taking_the_entry_out_leaves_the_rest_as_it_was() {
        let added = with_entry(TYPICAL, SID).unwrap();
        assert_eq!(without_entry(&added, SID).as_deref(), Some(TYPICAL));
        assert_eq!(without_entry(TYPICAL, SID), None);
        let lower = format!("D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(a;;cclcrpwplorc;;;{SID})S:(AU;FA;CC;;;WD)");
        assert_eq!(
            without_entry(&lower, SID).as_deref(),
            Some("D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)S:(AU;FA;CC;;;WD)")
        );
    }

    #[test]
    fn only_a_persons_sid_counts() {
        assert!(is_account_sid(SID));
        for sid in ["S-1-1-0", "S-1-5-32-545", "S-1-5-21-1-2", "S-1-5-21-a-b-c-d", "", "BU"] {
            assert!(!is_account_sid(sid), "{sid}");
        }
    }

    #[test]
    fn the_built_in_administrator_comes_back_as_la() {
        let admin = "S-1-5-21-1004336348-1177238915-682003330-500";
        let added = with_entry(TYPICAL, admin).unwrap();
        // Windows writes the computer's own Administrator as LA when it prints the permissions.
        let printed = added.replace(admin, "LA");
        assert!(has_entry(&printed, admin));
        assert_eq!(with_entry(&printed, admin), None);
        assert_eq!(without_entry(&printed, admin).as_deref(), Some(TYPICAL));
        assert!(!has_entry(&printed, SID), "only for the Administrator");
    }
}
