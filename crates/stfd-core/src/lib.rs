//! STFD's core: the apps it opens, the session that follows each one, the plan for what to
//! close after it, and the files it keeps. Everything outside `windows` is plain Rust and tested on
//! any system; `windows` reads and changes the real system.

pub mod candidates;
pub mod elevated;
pub mod model;
pub mod paths;
pub mod pe;
pub mod plan;
pub mod publisher;
pub mod sddl;
pub mod services;
pub mod session;
pub mod snapshot;
pub mod store;

#[cfg(windows)]
pub mod windows;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A short random id: 12 hex digits.
pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(now_ms());
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    format!("{:012x}", hasher.finish() & 0xffff_ffff_ffff)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_twelve_hex_digits_and_differ() {
        let a = super::new_id();
        let b = super::new_id();
        assert_eq!(a.len(), 12);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
