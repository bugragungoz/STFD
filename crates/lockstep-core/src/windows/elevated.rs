//! The elevated step itself: Lockstep started with administrator rights and a request on its
//! command line changes the named services, writes what it did to the result file and exits.

use super::{launch, services, SysError};
use crate::elevated::{decode, validate, Change, Outcome, Request};
use crate::model::{ServiceOriginal, StartType};
use crate::sddl;

fn apply(sid: &str, change: &Change) -> Outcome {
    let mut original = ServiceOriginal::default();
    let mut error: Option<String> = None;
    let mut fail = |what: &str, e: SysError| {
        error.get_or_insert(format!("{what}: {e}"));
    };

    if change.manual {
        match services::get_start_type(&change.name) {
            Ok(StartType::Manual) => {}
            Ok(before) => match services::set_start_type(&change.name, StartType::Manual) {
                Ok(()) => original.start_type = Some(before),
                Err(e) => fail("start type", e),
            },
            Err(e) => fail("start type", e),
        }
    }
    if let Some(start) = change.restore_start {
        if let Err(e) = services::set_start_type(&change.name, start) {
            fail("start type", e);
        }
    }
    if change.grant || change.revoke {
        match services::sddl(&change.name) {
            Ok(current) => {
                let new = if change.grant {
                    sddl::with_entry(&current, sid)
                } else {
                    sddl::without_entry(&current, sid)
                };
                let result = match &new {
                    Some(text) => services::set_sddl(&change.name, text),
                    None => Ok(()),
                };
                match result {
                    // Granted means the entry is there now, whoever put it there first.
                    Ok(()) => original.granted = change.grant,
                    Err(e) => fail("permissions", e),
                }
            }
            Err(e) => fail("permissions", e),
        }
    }
    Outcome {
        name: change.name.clone(),
        original,
        error,
    }
}

/// Carries out a request. The caller is elevated.
pub fn run(request: &Request) -> Vec<Outcome> {
    request.services.iter().map(|c| apply(&request.sid, c)).collect()
}

/// `lockstep.exe --elevated <hex request> <result file>`: the process exit code. 0 done (each
/// service's outcome is in the file), 2 the request is not valid, 3 not elevated, 4 the result could
/// not be written.
pub fn main(args: &[String]) -> i32 {
    let (Some(hex), Some(result_path)) = (args.first(), args.get(1)) else {
        return 2;
    };
    let Some(request) = decode(hex) else { return 2 };
    if validate(&request).is_err() {
        return 2;
    }
    if !launch::is_elevated() {
        return 3;
    }
    let outcomes = run(&request);
    match crate::store::write(std::path::Path::new(result_path), &outcomes) {
        Ok(()) => 0,
        Err(_) => 4,
    }
}
