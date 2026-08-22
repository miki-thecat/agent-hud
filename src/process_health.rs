//! Observation-only local process health.
//!
//! This module deliberately reports operating-system evidence separately from
//! session readiness. A process being present is useful diagnostic metadata,
//! but it does not establish that a particular Codex session is working or
//! waiting for the user.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Evidence {
    /// The process snapshot was read successfully.
    Available,
    /// The process snapshot could not be read at all.
    Unavailable,
    /// The snapshot was only partially readable.
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Running,
    NotRunning,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activity {
    /// No supported activity signal is available.
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessInfo {
    pub process_id: u32,
    pub executable: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessHealth {
    pub evidence: Evidence,
    pub presence: Presence,
    pub activity: Activity,
    pub processes: Vec<ProcessInfo>,
}

impl ProcessHealth {
    fn from_snapshot(evidence: Evidence, processes: Vec<ProcessInfo>) -> Self {
        let presence = match evidence {
            Evidence::Unavailable => Presence::Unknown,
            Evidence::Available | Evidence::Degraded if processes.is_empty() => {
                Presence::NotRunning
            }
            Evidence::Available | Evidence::Degraded => Presence::Running,
        };
        Self {
            evidence,
            presence,
            activity: Activity::Unknown,
            processes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessObserver {
    executable_names: Vec<String>,
}

impl ProcessObserver {
    /// Observe the default local Codex process name.
    pub fn codex() -> Self {
        Self::for_executables(["codex.exe"])
    }

    /// Observe a caller-provided set of executable names.
    pub fn for_executables<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            executable_names: names
                .into_iter()
                .map(|name| name.as_ref().trim().to_ascii_lowercase())
                .filter(|name| !name.is_empty())
                .collect(),
        }
    }

    /// Read a single best-effort process snapshot.
    ///
    /// This method only enumerates process metadata. It does not open, signal,
    /// terminate, inject into, or communicate with any process.
    pub fn observe(&self) -> ProcessHealth {
        platform::observe(&self.executable_names)
    }
}

#[cfg(windows)]
mod platform {
    use super::{ProcessHealth, ProcessInfo};
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_NO_MORE_FILES, GetLastError, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    pub fn observe(names: &[String]) -> ProcessHealth {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return ProcessHealth::from_snapshot(super::Evidence::Unavailable, Vec::new());
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        let first_ok = unsafe { Process32FirstW(snapshot, &mut entry) != 0 };
        if !first_ok {
            unsafe { CloseHandle(snapshot) };
            return ProcessHealth::from_snapshot(super::Evidence::Degraded, Vec::new());
        }

        let mut processes = Vec::new();
        let mut evidence = super::Evidence::Available;
        loop {
            let executable = wide_name(&entry.szExeFile);
            if names
                .iter()
                .any(|name| name == &executable.to_ascii_lowercase())
            {
                processes.push(ProcessInfo {
                    process_id: entry.th32ProcessID,
                    executable,
                });
            }
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                // Toolhelp uses a zero return for both end-of-snapshot and an
                // enumeration error. Preserve the records already observed,
                // but report degraded evidence when the error is not the
                // normal end-of-snapshot condition.
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    evidence = super::Evidence::Degraded;
                }
                break;
            }
        }
        unsafe { CloseHandle(snapshot) };
        ProcessHealth::from_snapshot(evidence, processes)
    }

    fn wide_name(value: &[u16]) -> String {
        let end = value
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..end])
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{Evidence, ProcessHealth};

    pub fn observe(_names: &[String]) -> ProcessHealth {
        ProcessHealth::from_snapshot(Evidence::Unavailable, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, Evidence, Presence, ProcessHealth, ProcessInfo, ProcessObserver};

    #[test]
    fn unavailable_evidence_does_not_claim_process_presence() {
        let health = ProcessHealth::from_snapshot(Evidence::Unavailable, Vec::new());
        assert_eq!(health.presence, Presence::Unknown);
        assert_eq!(health.activity, Activity::Unknown);
        assert!(health.processes.is_empty());
    }

    #[test]
    fn degraded_evidence_can_report_observed_processes_without_activity_claim() {
        let health = ProcessHealth::from_snapshot(
            Evidence::Degraded,
            vec![ProcessInfo {
                process_id: 42,
                executable: "codex.exe".into(),
            }],
        );
        assert_eq!(health.presence, Presence::Running);
        assert_eq!(health.activity, Activity::Unknown);
        assert_eq!(health.processes[0].process_id, 42);
    }

    #[test]
    fn observer_normalizes_executable_names_without_empty_targets() {
        let observer = ProcessObserver::for_executables([" CODEX.EXE ", ""]);
        assert_eq!(observer.executable_names, vec!["codex.exe".to_owned()]);
    }
}
