//! Carrying out a plan: starting an app's services before it opens, and closing what it left
//! after: programs are asked to close first and ended only if they do not.

use super::process::{ask_to_close, Held};
use super::{services, SysError};
use crate::model::Problem;
use crate::plan::{Plan, Target};
use crate::snapshot::Service;
use std::time::{Duration, Instant};

/// How long a service may take to start or stop.
pub const SERVICE_TIMEOUT: Duration = Duration::from_secs(30);

fn code(e: &SysError, otherwise: &str) -> String {
    match e {
        SysError::AccessDenied => "access_denied".into(),
        _ => otherwise.into(),
    }
}

fn problem(what: &str, e: &SysError, otherwise: &str) -> Problem {
    Problem {
        what: what.to_string(),
        code: code(e, otherwise),
        detail: e.to_string(),
    }
}

fn display(services: &[Service], name: &str) -> String {
    services
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .map(|s| s.display.clone())
        .unwrap_or_else(|| name.to_string())
}

/// What starting did.
#[derive(Debug, Default)]
pub struct Started {
    /// Display names of the services that started.
    pub started: Vec<String>,
    pub problems: Vec<Problem>,
}

/// Starts the services in order and waits for each to run.
pub fn start_services(names: &[String], known: &[Service]) -> Started {
    let mut out = Started::default();
    for name in names {
        let shown = display(known, name);
        match services::start(name, SERVICE_TIMEOUT) {
            Ok(()) => out.started.push(shown),
            Err(e) => out.problems.push(problem(&shown, &e, "start_failed")),
        }
    }
    out
}

/// What closing did.
#[derive(Debug, Default)]
pub struct Closed {
    /// Services that stopped (display names).
    pub stopped: Vec<String>,
    /// Programs that closed when asked.
    pub closed: Vec<String>,
    /// Programs that had to be ended.
    pub ended: Vec<String>,
    pub problems: Vec<Problem>,
}

/// Closes what the plan lists: asks the programs to close, stops the services (whose own processes
/// go with them), gives the programs `polite` to finish, ends the rest. Then, after a moment, ends
/// what `again` still finds (programs a service or updater started again on the way out).
pub fn close(plan: &Plan, known: &[Service], polite: Duration, again: impl Fn() -> Vec<Target>) -> Closed {
    let mut out = Closed::default();

    let mut held: Vec<Held> = Vec::new();
    for t in &plan.programs {
        match Held::open(t.pid, &t.path) {
            Ok(h) => held.push(h),
            // Gone already, or a different program by now: nothing to do.
            Err(SysError::Other(_)) => {}
            Err(e) => out.problems.push(problem(&t.name, &e, "close_failed")),
        }
    }
    for h in &held {
        ask_to_close(h.pid);
    }

    for name in &plan.services {
        let shown = display(known, name);
        match services::stop(name, SERVICE_TIMEOUT) {
            Ok(()) => out.stopped.push(shown),
            Err(e) => out.problems.push(problem(&shown, &e, "stop_failed")),
        }
    }

    let deadline = Instant::now() + polite;
    while Instant::now() < deadline && held.iter().any(|h| !h.exited()) {
        std::thread::sleep(Duration::from_millis(100));
    }
    for h in &held {
        if h.exited() {
            out.closed.push(h.name.clone());
            continue;
        }
        match h.end() {
            Ok(()) => {
                h.wait(5_000);
                out.ended.push(h.name.clone());
            }
            Err(e) => out.problems.push(problem(&h.name, &e, "close_failed")),
        }
    }
    drop(held);

    // One more look: anything that came back is ended without asking.
    std::thread::sleep(Duration::from_millis(1_500));
    for t in again() {
        let Ok(h) = Held::open(t.pid, &t.path) else { continue };
        match h.end() {
            Ok(()) => {
                h.wait(5_000);
                out.ended.push(t.name.clone());
            }
            Err(e) => out.problems.push(problem(&t.name, &e, "close_failed")),
        }
    }
    out
}
