//! Services through the Service Control Manager: their states, their configuration, starting and
//! stopping them, and (as administrator) their permissions and start type.

use super::{from_wide_ptr, version, wide, SysError, SysResult};
use crate::model::StartType;
use crate::paths::KnownFolders;
use crate::services::{program_of, ServiceInfo};
use crate::snapshot::{Service, SvcState};
use std::time::{Duration, Instant};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    LocalFree, ERROR_INSUFFICIENT_BUFFER, ERROR_MORE_DATA, ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_NOT_ACTIVE,
    HLOCAL,
};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    SDDL_REVISION_1,
};
use windows::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
use windows::Win32::Storage::FileSystem::{READ_CONTROL, WRITE_DAC};
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
    QueryServiceConfigW, QueryServiceObjectSecurity, QueryServiceStatusEx, SetServiceObjectSecurity, StartServiceW,
    ENUM_SERVICE_STATUS_PROCESSW, ENUM_SERVICE_TYPE, QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO, SC_HANDLE,
    SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SC_STATUS_PROCESS_INFO, SERVICE_AUTO_START, SERVICE_BOOT_START,
    SERVICE_CHANGE_CONFIG, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_ERROR,
    SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_START_TYPE,
    SERVICE_STATE_ALL, SERVICE_STATUS, SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_PROCESS, SERVICE_STOP,
    SERVICE_SYSTEM_START, SERVICE_WIN32,
};

/// A service-manager handle that closes itself.
struct Sc(SC_HANDLE);

impl Drop for Sc {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn manager() -> SysResult<Sc> {
    unsafe {
        Ok(Sc(OpenSCManagerW(
            PCWSTR::null(),
            PCWSTR::null(),
            SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE,
        )?))
    }
}

fn open(scm: &Sc, name: &str, access: u32) -> SysResult<Sc> {
    let w = wide(name);
    unsafe { Ok(Sc(OpenServiceW(scm.0, PCWSTR(w.as_ptr()), access)?)) }
}

fn state(s: SERVICE_STATUS_CURRENT_STATE) -> SvcState {
    match s.0 {
        1 => SvcState::Stopped,
        2 => SvcState::Starting,
        3 => SvcState::Stopping,
        4 => SvcState::Running,
        5 | 6 => SvcState::Running,
        _ => SvcState::Paused,
    }
}

fn start_type(t: SERVICE_START_TYPE) -> StartType {
    match t {
        SERVICE_BOOT_START => StartType::Boot,
        SERVICE_SYSTEM_START => StartType::System,
        SERVICE_AUTO_START => StartType::Automatic,
        SERVICE_DISABLED => StartType::Disabled,
        _ => StartType::Manual,
    }
}

/// Every program service (not drivers) with its state and process.
pub fn states() -> SysResult<Vec<Service>> {
    let scm = manager()?;
    let mut buffer: Vec<u8> = vec![0; 64 * 1024];
    let mut out = Vec::new();
    let mut resume = 0u32;
    loop {
        let mut needed = 0u32;
        let mut count = 0u32;
        let result = unsafe {
            EnumServicesStatusExW(
                scm.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_STATE_ALL,
                Some(buffer.as_mut_slice()),
                &mut needed,
                &mut count,
                Some(&mut resume as *mut u32),
                PCWSTR::null(),
            )
        };
        let more = match &result {
            Ok(()) => false,
            Err(e) if e.code() == ERROR_MORE_DATA.to_hresult() => true,
            Err(e) => return Err(e.clone().into()),
        };
        let entries = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW, count as usize)
        };
        for e in entries {
            out.push(Service {
                name: unsafe { from_wide_ptr(e.lpServiceName.0) },
                display: unsafe { from_wide_ptr(e.lpDisplayName.0) },
                state: state(e.ServiceStatusProcess.dwCurrentState),
                pid: e.ServiceStatusProcess.dwProcessId,
                depends_on: Vec::new(),
            });
        }
        if !more {
            break;
        }
        if needed as usize > buffer.len() {
            buffer.resize(needed as usize, 0);
        }
    }
    Ok(out)
}

struct Config {
    binary: String,
    start: StartType,
    depends_on: Vec<String>,
}

fn config(service: &Sc) -> SysResult<Config> {
    let mut needed = 0u32;
    unsafe {
        let first = QueryServiceConfigW(service.0, None, 0, &mut needed);
        if let Err(e) = first {
            if e.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() {
                return Err(e.into());
            }
        }
        // u64 storage keeps the struct aligned.
        let mut buffer: Vec<u64> = vec![0; (needed as usize).div_ceil(8).max(1)];
        let cfg = buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
        QueryServiceConfigW(service.0, Some(cfg), (buffer.len() * 8) as u32, &mut needed)?;
        let cfg = &*cfg;
        let mut depends_on = Vec::new();
        // A double-null-terminated list; names starting with '+' are groups.
        let mut p = cfg.lpDependencies.0 as *const u16;
        if !p.is_null() {
            loop {
                let item = from_wide_ptr(p);
                if item.is_empty() {
                    break;
                }
                p = p.add(item.encode_utf16().count() + 1);
                if !item.starts_with('+') {
                    depends_on.push(item);
                }
            }
        }
        Ok(Config {
            binary: from_wide_ptr(cfg.lpBinaryPathName.0),
            start: start_type(cfg.dwStartType),
            depends_on,
        })
    }
}

/// The services' states with what each needs, for planning a stop.
pub fn states_with_dependencies() -> SysResult<Vec<Service>> {
    let mut all = states()?;
    let scm = manager()?;
    for s in all.iter_mut().filter(|s| s.state.is_up()) {
        if let Ok(service) = open(&scm, &s.name, SERVICE_QUERY_CONFIG) {
            if let Ok(cfg) = config(&service) {
                s.depends_on = cfg.depends_on;
            }
        }
    }
    Ok(all)
}

/// The DLL of a service that runs inside a shared host, from its Parameters\ServiceDll value.
fn service_dll(name: &str) -> Option<String> {
    // REG_SZ also reads REG_EXPAND_SZ values, with %variables% expanded.
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let key = wide(&format!("SYSTEM\\CurrentControlSet\\Services\\{name}\\Parameters"));
    let value = wide("ServiceDll");
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size as *mut u32),
        )
    };
    r.is_ok().then(|| super::from_wide(&buf)).filter(|s| !s.is_empty())
}

/// Every program service with its program, publisher and whether this person may start and stop
/// it, for choosing an app's services. Reads every service's configuration: a second or so.
pub fn list(known: &KnownFolders) -> SysResult<Vec<ServiceInfo>> {
    let scm = manager()?;
    let mut companies: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for s in states()? {
        let Ok(service) = open(&scm, &s.name, SERVICE_QUERY_CONFIG) else {
            continue;
        };
        let Ok(cfg) = config(&service) else { continue };
        let program = program_of(&cfg.binary, &known.windows, |p| std::path::Path::new(p).is_file());
        let dll = program
            .as_deref()
            .filter(|p| crate::paths::file_name(p).eq_ignore_ascii_case("svchost.exe"))
            .and_then(|_| service_dll(&s.name));
        let source = dll.clone().or_else(|| program.clone());
        let company = source.and_then(|p| {
            companies
                .entry(crate::paths::normalize(&p))
                .or_insert_with(|| version::strings(&p).company)
                .clone()
        });
        out.push(ServiceInfo {
            controllable: open(&scm, &s.name, SERVICE_START | SERVICE_STOP | SERVICE_QUERY_STATUS).is_ok(),
            name: s.name,
            display: s.display,
            state: s.state,
            start_type: cfg.start,
            program,
            dll,
            company,
        });
    }
    Ok(out)
}

/// True when this process may start and stop the service.
pub fn controllable(name: &str) -> bool {
    manager()
        .and_then(|scm| open(&scm, name, SERVICE_START | SERVICE_STOP | SERVICE_QUERY_STATUS))
        .is_ok()
}

fn current(service: &Sc) -> SysResult<SERVICE_STATUS_PROCESS> {
    let mut status = SERVICE_STATUS_PROCESS::default();
    let mut needed = 0u32;
    unsafe {
        let bytes = std::slice::from_raw_parts_mut(
            &mut status as *mut _ as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
        );
        QueryServiceStatusEx(service.0, SC_STATUS_PROCESS_INFO, Some(bytes), &mut needed)?;
    }
    Ok(status)
}

fn wait_for(service: &Sc, want: SvcState, timeout: Duration) -> SysResult<()> {
    let started = Instant::now();
    loop {
        let now = state(current(service)?.dwCurrentState);
        if now == want {
            return Ok(());
        }
        // A service that stopped while starting failed to start.
        if want == SvcState::Running && now == SvcState::Stopped && started.elapsed() > Duration::from_millis(500) {
            return Err(SysError::Other("it stopped again".into()));
        }
        if started.elapsed() >= timeout {
            return Err(SysError::Other(format!(
                "it did not finish within {} s",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Starts a service and waits until it runs.
pub fn start(name: &str, timeout: Duration) -> SysResult<()> {
    let scm = manager()?;
    let service = open(&scm, name, SERVICE_START | SERVICE_QUERY_STATUS)?;
    match unsafe { StartServiceW(service.0, None) } {
        Ok(()) => {}
        Err(e) if e.code() == ERROR_SERVICE_ALREADY_RUNNING.to_hresult() => {}
        Err(e) => return Err(e.into()),
    }
    wait_for(&service, SvcState::Running, timeout)
}

/// Stops a service and waits until it has stopped.
pub fn stop(name: &str, timeout: Duration) -> SysResult<()> {
    let scm = manager()?;
    let service = open(&scm, name, SERVICE_STOP | SERVICE_QUERY_STATUS)?;
    let mut status = SERVICE_STATUS::default();
    match unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) } {
        Ok(()) => {}
        Err(e) if e.code() == ERROR_SERVICE_NOT_ACTIVE.to_hresult() => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    wait_for(&service, SvcState::Stopped, timeout)
}

/// A service's permissions as SDDL (its DACL).
pub fn sddl(name: &str) -> SysResult<String> {
    let scm = manager()?;
    let service = open(&scm, name, READ_CONTROL.0)?;
    unsafe {
        let mut needed = 0u32;
        let _ = QueryServiceObjectSecurity(service.0, DACL_SECURITY_INFORMATION.0, None, 0, &mut needed);
        let mut buffer: Vec<u64> = vec![0; (needed as usize).div_ceil(8).max(1)];
        let sd = PSECURITY_DESCRIPTOR(buffer.as_mut_ptr().cast());
        QueryServiceObjectSecurity(
            service.0,
            DACL_SECURITY_INFORMATION.0,
            Some(sd),
            (buffer.len() * 8) as u32,
            &mut needed,
        )?;
        let mut text = PWSTR::null();
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            None,
        )?;
        let out = from_wide_ptr(text.0);
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(out)
    }
}

/// Sets a service's permissions (its DACL) from SDDL. Needs administrator rights.
pub fn set_sddl(name: &str, sddl: &str) -> SysResult<()> {
    let scm = manager()?;
    let service = open(&scm, name, WRITE_DAC.0 | READ_CONTROL.0)?;
    let text = wide(sddl);
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(text.as_ptr()), SDDL_REVISION_1, &mut sd, None)?;
        let result = SetServiceObjectSecurity(service.0, DACL_SECURITY_INFORMATION, sd);
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        result?;
    }
    Ok(())
}

pub fn get_start_type(name: &str) -> SysResult<StartType> {
    let scm = manager()?;
    let service = open(&scm, name, SERVICE_QUERY_CONFIG)?;
    Ok(config(&service)?.start)
}

/// Sets when Windows starts the service. Needs administrator rights.
pub fn set_start_type(name: &str, start: StartType) -> SysResult<()> {
    let value = match start {
        StartType::Automatic => SERVICE_AUTO_START,
        StartType::Manual => SERVICE_DEMAND_START,
        StartType::Disabled => SERVICE_DISABLED,
        StartType::Boot | StartType::System => return Err(SysError::Other("not a start type for a service".into())),
    };
    let scm = manager()?;
    let service = open(&scm, name, SERVICE_CHANGE_CONFIG)?;
    unsafe {
        ChangeServiceConfigW(
            service.0,
            ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
            value,
            SERVICE_ERROR(SERVICE_NO_CHANGE),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        )?;
    }
    Ok(())
}

/// The SID of the account this process runs as (`S-1-5-21-...`).
pub fn own_sid() -> SysResult<String> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buffer: Vec<u64> = vec![0; (needed as usize).div_ceil(8).max(1)];
        let result = GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            (buffer.len() * 8) as u32,
            &mut needed,
        );
        let _ = CloseHandle(token);
        result?;
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text)?;
        let out = from_wide_ptr(text.0);
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(out)
    }
}
