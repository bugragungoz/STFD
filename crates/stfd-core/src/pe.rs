//! Whether a program has windows or runs in a console, from the start of its file (the PE header).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subsystem {
    /// A program with windows.
    Gui,
    Console,
    Other,
}

/// Reads the subsystem from the first bytes of an .exe (4 KB is plenty); None when it is not a
/// Windows program.
pub fn subsystem(head: &[u8]) -> Option<Subsystem> {
    if head.get(..2)? != b"MZ" {
        return None;
    }
    let pe = u32::from_le_bytes(head.get(0x3c..0x40)?.try_into().ok()?) as usize;
    if head.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let optional = pe + 24;
    let magic = u16::from_le_bytes(head.get(optional..optional + 2)?.try_into().ok()?);
    if magic != 0x10b && magic != 0x20b {
        return None;
    }
    // The subsystem sits 68 bytes into the optional header in both PE32 and PE32+.
    let value = u16::from_le_bytes(head.get(optional + 68..optional + 70)?.try_into().ok()?);
    Some(match value {
        2 => Subsystem::Gui,
        3 => Subsystem::Console,
        _ => Subsystem::Other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(magic: u16, subsystem: u16) -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        let optional = 0x80 + 24;
        b[optional..optional + 2].copy_from_slice(&magic.to_le_bytes());
        b[optional + 68..optional + 70].copy_from_slice(&subsystem.to_le_bytes());
        b
    }

    #[test]
    fn window_and_console_programs_in_32_and_64_bit() {
        assert_eq!(subsystem(&exe(0x20b, 2)), Some(Subsystem::Gui));
        assert_eq!(subsystem(&exe(0x10b, 3)), Some(Subsystem::Console));
        assert_eq!(subsystem(&exe(0x20b, 10)), Some(Subsystem::Other));
    }

    #[test]
    fn other_files_are_not_programs() {
        assert_eq!(subsystem(b"not a program"), None);
        assert_eq!(subsystem(b"MZ"), None);
        assert_eq!(subsystem(&exe(0x999, 2)), None);
        let mut cut = exe(0x20b, 2);
        cut.truncate(0x90);
        assert_eq!(subsystem(&cut), None);
    }
}
