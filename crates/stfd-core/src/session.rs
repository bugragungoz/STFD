//! One opening of an app, followed step by step: starting, open, closing (the wait after the
//! program closed, which reopening cancels), then cleaning up.

use crate::model::Trigger;
use serde::Serialize;

/// How long a launched program may take to show up before STFD treats it as closed.
pub const LAUNCH_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Phase {
    /// Services are starting or the program was launched and has not shown up yet.
    Starting {
        since: u64,
    },
    Open {
        since: u64,
    },
    /// The program closed; the rest closes at `at` unless it opens again.
    Closing {
        since: u64,
        at: u64,
    },
    /// Closing what was left.
    Cleaning,
}

/// What a step changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    None,
    Opened,
    Closing {
        at: u64,
    },
    Reopened,
    /// Time to close what is left.
    Cleanup,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub app_id: String,
    pub trigger: Trigger,
    pub phase: Phase,
    /// When the program was first seen open.
    pub opened_at: Option<u64>,
    /// Services STFD started for this session (display names).
    pub started: Vec<String>,
    /// Programs of the app seen running during the session (paths).
    pub seen: Vec<String>,
}

impl Session {
    /// Opened by STFD: services start, then the program.
    pub fn launched(app_id: &str, now: u64) -> Self {
        Self {
            app_id: app_id.to_string(),
            trigger: Trigger::Shortcut,
            phase: Phase::Starting { since: now },
            opened_at: None,
            started: Vec::new(),
            seen: Vec::new(),
        }
    }

    /// Opened another way and noticed already open.
    pub fn adopted(app_id: &str, now: u64) -> Self {
        Self {
            trigger: Trigger::Adopted,
            phase: Phase::Open { since: now },
            opened_at: Some(now),
            ..Self::launched(app_id, now)
        }
    }

    /// One step: `open` is whether the app is open now.
    pub fn step(&mut self, open: bool, now: u64, wait_ms: u64) -> Change {
        match self.phase {
            Phase::Starting { since } => {
                if open {
                    self.phase = Phase::Open { since: now };
                    self.opened_at.get_or_insert(now);
                    Change::Opened
                } else if now.saturating_sub(since) >= LAUNCH_TIMEOUT_MS {
                    self.closing(now, wait_ms)
                } else {
                    Change::None
                }
            }
            Phase::Open { .. } => {
                if open {
                    Change::None
                } else {
                    self.closing(now, wait_ms)
                }
            }
            Phase::Closing { at, .. } => {
                if open {
                    self.phase = Phase::Open { since: now };
                    Change::Reopened
                } else if now >= at {
                    self.phase = Phase::Cleaning;
                    Change::Cleanup
                } else {
                    Change::None
                }
            }
            Phase::Cleaning => Change::None,
        }
    }

    fn closing(&mut self, now: u64, wait_ms: u64) -> Change {
        let at = now.saturating_add(wait_ms);
        self.phase = Phase::Closing { since: now, at };
        Change::Closing { at }
    }

    /// The program is being opened again (the shortcut while closing): wait for it anew.
    pub fn relaunched(&mut self, now: u64) {
        if !matches!(self.phase, Phase::Cleaning | Phase::Open { .. }) {
            self.phase = Phase::Starting { since: now };
        }
    }

    /// The program could not be opened: close what was started without waiting.
    pub fn launch_failed(&mut self, now: u64) {
        self.phase = Phase::Closing { since: now, at: now };
    }

    /// "Close now" while waiting: the next step cleans up.
    pub fn close_now(&mut self, now: u64) -> bool {
        if let Phase::Closing { since, .. } = self.phase {
            self.phase = Phase::Closing { since, at: now };
            true
        } else {
            false
        }
    }

    pub fn is_cleaning(&self) -> bool {
        self.phase == Phase::Cleaning
    }

    pub fn note_seen(&mut self, paths: impl IntoIterator<Item = String>) {
        for p in paths {
            if !self.seen.iter().any(|s| crate::paths::same(s, &p)) {
                self.seen.push(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launched_opens_closes_after_the_wait_and_cleans_up() {
        let mut s = Session::launched("a", 0);
        assert_eq!(s.step(false, 1_000, 30_000), Change::None);
        assert_eq!(s.step(true, 2_000, 30_000), Change::Opened);
        assert_eq!(s.opened_at, Some(2_000));
        assert_eq!(s.step(true, 60_000, 30_000), Change::None);
        assert_eq!(s.step(false, 61_000, 30_000), Change::Closing { at: 91_000 });
        assert_eq!(s.step(false, 90_999, 30_000), Change::None);
        assert_eq!(s.step(false, 91_000, 30_000), Change::Cleanup);
        assert!(s.is_cleaning());
        assert_eq!(s.step(false, 92_000, 30_000), Change::None, "cleaning happens once");
    }

    #[test]
    fn opening_again_while_waiting_cancels_the_cleanup() {
        let mut s = Session::adopted("a", 0);
        assert_eq!(s.step(false, 1_000, 30_000), Change::Closing { at: 31_000 });
        assert_eq!(s.step(true, 20_000, 30_000), Change::Reopened);
        assert_eq!(s.phase, Phase::Open { since: 20_000 });
        assert_eq!(
            s.step(false, 25_000, 30_000),
            Change::Closing { at: 55_000 },
            "the wait starts over"
        );
    }

    #[test]
    fn a_program_that_never_shows_up_counts_as_closed() {
        let mut s = Session::launched("a", 0);
        assert_eq!(s.step(false, LAUNCH_TIMEOUT_MS - 1, 10_000), Change::None);
        assert_eq!(
            s.step(false, LAUNCH_TIMEOUT_MS, 10_000),
            Change::Closing {
                at: LAUNCH_TIMEOUT_MS + 10_000
            }
        );
        assert_eq!(s.opened_at, None);
    }

    #[test]
    fn no_wait_cleans_up_on_the_next_step() {
        let mut s = Session::adopted("a", 0);
        assert_eq!(s.step(false, 1_000, 0), Change::Closing { at: 1_000 });
        assert_eq!(s.step(false, 2_000, 0), Change::Cleanup);
    }

    #[test]
    fn close_now_and_a_failed_launch_skip_the_wait() {
        let mut s = Session::adopted("a", 0);
        assert!(!s.close_now(5), "nothing to hurry while open");
        s.step(false, 1_000, 60_000);
        assert!(s.close_now(2_000));
        assert_eq!(s.step(false, 2_000, 60_000), Change::Cleanup);

        let mut s = Session::launched("b", 0);
        s.launch_failed(500);
        assert_eq!(s.step(false, 600, 60_000), Change::Cleanup);
    }

    #[test]
    fn the_shortcut_while_waiting_waits_for_the_program_again() {
        let mut s = Session::adopted("a", 0);
        s.step(false, 1_000, 60_000);
        s.relaunched(2_000);
        assert_eq!(s.phase, Phase::Starting { since: 2_000 });
        assert_eq!(s.step(true, 3_000, 60_000), Change::Opened);
        assert_eq!(s.opened_at, Some(0), "first opening is kept");
        s.relaunched(4_000);
        assert_eq!(s.phase, Phase::Open { since: 3_000 }, "already open");
    }

    #[test]
    fn seen_programs_are_listed_once() {
        let mut s = Session::launched("a", 0);
        s.note_seen([
            "C:\\A\\a.exe".to_string(),
            "c:\\a\\A.exe".to_string(),
            "C:\\A\\b.exe".to_string(),
        ]);
        assert_eq!(s.seen, vec!["C:\\A\\a.exe", "C:\\A\\b.exe"]);
    }
}
