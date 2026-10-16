//! The one step that needs administrator rights: letting the person start and stop an app's
//! services, and setting them to start by hand. Lockstep runs itself elevated with a request on its
//! command line (`--elevated <hex> <result file>`); Windows asks the person first. This module is
//! the request and its checks; `windows::elevated` carries it out.

use crate::model::{ServiceOriginal, StartType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// The person's account. The elevated copy may run as another (administrator) account, so it
    /// cannot read this from its own token.
    pub sid: String,
    pub services: Vec<Change>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Change {
    pub name: String,
    /// Let the person start and stop it.
    pub grant: bool,
    /// Take that permission out again.
    pub revoke: bool,
    /// Set it to start by hand (Manual), remembering how it started.
    pub manual: bool,
    /// Put this start type back.
    pub restore_start: Option<StartType>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub name: String,
    /// What changed, for the app to remember (and undo later).
    pub original: ServiceOriginal,
    pub error: Option<String>,
}

const MAX_SERVICES: usize = 200;

/// Checks a request before anything changes: a person's SID, real-looking service names, start
/// types a service may have.
pub fn validate(request: &Request) -> Result<(), String> {
    if !crate::sddl::is_account_sid(&request.sid) {
        return Err(format!("not a person's account: {}", request.sid));
    }
    if request.services.is_empty() || request.services.len() > MAX_SERVICES {
        return Err("no services, or too many".into());
    }
    for c in &request.services {
        if c.name.is_empty() || c.name.len() > 256 || c.name.contains(['/', '\\']) {
            return Err(format!("not a service name: {}", c.name));
        }
        if c.grant && c.revoke {
            return Err(format!("both grant and revoke for {}", c.name));
        }
        if matches!(c.restore_start, Some(StartType::Boot | StartType::System)) {
            return Err(format!("not a start type for a service: {}", c.name));
        }
    }
    Ok(())
}

/// The request as hex digits of its JSON: safe on a command line, no quoting.
pub fn encode(request: &Request) -> String {
    let json = serde_json::to_vec(request).unwrap_or_default();
    json.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn decode(hex: &str) -> Option<Request> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    serde_json::from_slice(&bytes?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "S-1-5-21-1004336348-1177238915-682003330-1001";

    fn request() -> Request {
        Request {
            sid: SID.into(),
            services: vec![Change {
                name: "NI Domain Service".into(),
                grant: true,
                manual: true,
                ..Change::default()
            }],
        }
    }

    #[test]
    fn a_request_survives_the_command_line() {
        let r = request();
        let hex = encode(&r);
        assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(decode(&hex), Some(r));
        assert_eq!(decode("abc"), None);
        assert_eq!(decode("zz"), None);
    }

    #[test]
    fn only_sensible_requests_pass() {
        assert_eq!(validate(&request()), Ok(()));
        let mut r = request();
        r.sid = "S-1-1-0".into();
        assert!(validate(&r).is_err(), "Everyone");
        let mut r = request();
        r.services.clear();
        assert!(validate(&r).is_err());
        let mut r = request();
        r.services[0].name = "..\\x".into();
        assert!(validate(&r).is_err());
        let mut r = request();
        r.services[0].revoke = true;
        assert!(validate(&r).is_err(), "grant and revoke");
        let mut r = request();
        r.services[0].restore_start = Some(StartType::Boot);
        assert!(validate(&r).is_err());
    }
}
