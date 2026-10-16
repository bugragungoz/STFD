//! What runs right now: processes and services, as the Windows side reads them each step.

use serde::{Deserialize, Serialize};

/// A running process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proc {
    pub pid: u32,
    pub parent: u32,
    /// The program's file name (`multisim.exe`).
    pub name: String,
    /// The program's full path; None when Windows does not say (System, protected processes).
    pub path: Option<String>,
    /// It shows a window on the desktop.
    pub windowed: bool,
    /// The Windows session it runs in: 0 for services, the user's own for their programs.
    pub session: u32,
}

/// A service's state, as Windows reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SvcState {
    Stopped,
    Starting,
    Stopping,
    Running,
    Paused,
}

impl SvcState {
    /// True when stopping it would change something.
    pub fn is_up(self) -> bool {
        !matches!(self, SvcState::Stopped | SvcState::Stopping)
    }
}

/// A service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub name: String,
    pub display: String,
    pub state: SvcState,
    /// The process it runs in, 0 when stopped.
    pub pid: u32,
    /// The services it needs (key names). Filled when STFD plans a stop; empty otherwise.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub procs: Vec<Proc>,
    pub services: Vec<Service>,
}

impl Snapshot {
    pub fn service(&self, name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// The running services that live in this process (a shared host runs several).
    pub fn services_in(&self, pid: u32) -> impl Iterator<Item = &Service> {
        self.services
            .iter()
            .filter(move |s| pid != 0 && s.pid == pid && s.state.is_up())
    }
}
