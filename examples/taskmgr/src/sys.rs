//! Cross-platform system sampling for the Task Manager.
//!
//! The Linux backend reads `/proc` and `/sys` with pure `std`; the Windows
//! backend uses `windows-sys` (PSAPI/Toolhelp/IP Helper). Both implement the
//! same [`Sampler`] contract; the backend is selected with `cfg`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Estimated power draw of a process, derived from CPU usage.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PowerLevel {
    #[default]
    Low,
    Medium,
    High,
    VeryHigh,
}

impl PowerLevel {
    /// The Chinese label shown in the 电源使用情况 column.
    pub fn label(self) -> &'static str {
        match self {
            PowerLevel::Low => "低",
            PowerLevel::Medium => "中",
            PowerLevel::High => "高",
            PowerLevel::VeryHigh => "非常高",
        }
    }

    fn from_cpu(cpu_pct: f64) -> Self {
        if cpu_pct >= 30.0 {
            PowerLevel::VeryHigh
        } else if cpu_pct >= 10.0 {
            PowerLevel::High
        } else if cpu_pct >= 1.0 {
            PowerLevel::Medium
        } else {
            PowerLevel::Low
        }
    }
}

/// Task-Manager-style grouping of a process.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ProcessGroup {
    #[default]
    App,
    Background,
    System,
}

impl ProcessGroup {
    /// The Chinese label shown in the 类型 column.
    pub fn label(self) -> &'static str {
        match self {
            ProcessGroup::App => "应用",
            ProcessGroup::Background => "后台进程",
            ProcessGroup::System => "系统进程",
        }
    }
}

/// One row of the process table.
#[derive(Clone, PartialEq, Default)]
pub struct ProcessSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    /// Absolute path of the executable, when resolvable.
    pub exe: String,
    pub threads: u32,
    /// Accumulated CPU time in seconds.
    pub cpu_seconds: f64,
    /// Process start identity (creation time) used to guard async actions
    /// such as terminating a process after a confirmation dialog.
    pub start_time: u64,
    /// Human-readable priority class (高/普通/低/…).
    pub priority: String,
    pub group: ProcessGroup,
    /// CPU usage as a percentage of total machine capacity (0..100).
    pub cpu: f64,
    pub mem_bytes: u64,
    pub disk_bytes_per_s: u64,
    pub net_bytes_per_s: u64,
    pub power: PowerLevel,
}

/// A network interface with its current transfer rates.
#[derive(Clone, PartialEq)]
pub struct NetRate {
    pub name: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
}

/// CPU identity and live usage.
#[derive(Clone, PartialEq)]
pub struct CpuInfo {
    pub usage_pct: f64,
    pub per_core_pct: Vec<f64>,
    pub current_mhz: f64,
    pub base_mhz: f64,
    pub sockets: u32,
    pub cores: u32,
    pub logical: u32,
    pub virtualization: bool,
    pub l1_kb: u64,
    pub l2_kb: u64,
    pub l3_kb: u64,
}

/// One complete sampling pass.
#[derive(Clone, PartialEq)]
pub struct SystemStats {
    pub processes: Vec<ProcessSample>,
    pub cpu: CpuInfo,
    pub mem_total: u64,
    pub mem_used: u64,
    pub uptime_secs: u64,
    pub process_count: u32,
    pub thread_count: u32,
    pub handle_count: u32,
    pub nets: Vec<NetRate>,
    pub disk_names: Vec<String>,
    /// Per-disk active time percentage, when the platform provides it.
    pub disk_activity: Vec<(String, f64)>,
    pub gpu_name: String,
}

/// A row of the Services page: `(name, state, description)`.
pub type ServiceEntry = (String, String, String);

/// A row of the Startup page: `(name, command, state)`.
pub type StartupEntry = (String, String, String);

/// A row of the Users page: `(user, session, state)`.
pub type UserSession = (String, String, String);

#[cfg(target_os = "linux")]
type Backend = linux::LinuxSampler;
#[cfg(target_os = "windows")]
type Backend = windows::WindowsSampler;

#[cfg(target_os = "linux")]
pub use linux::{service_entries, startup_entries, user_sessions};
#[cfg(target_os = "windows")]
pub use windows::{service_entries, startup_entries, user_sessions};

/// Samples the system; call [`Sampler::sample`] once per refresh tick.
pub struct Sampler {
    backend: Backend,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            backend: Backend::new(),
        }
    }

    pub fn sample(&mut self) -> SystemStats {
        self.backend.sample()
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminate `pid`, but only if its start identity still matches
/// `expected_start_time` — protecting against PID reuse between the moment a
/// row was selected and the confirmation dialog is answered. Refuses to kill
/// the current process.
pub fn kill_process(pid: u32, expected_start_time: u64) -> bool {
    if pid == std::process::id() {
        return false;
    }
    kill_impl(pid, expected_start_time)
}

#[cfg(target_os = "linux")]
fn kill_impl(pid: u32, expected_start_time: u64) -> bool {
    let current = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            let close = stat.rfind(')')?;
            let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
            // Field 22 (starttime) is rest[19].
            rest.get(19)?.parse().ok()
        })
        .unwrap_or(0);
    if current != expected_start_time {
        return false; // the pid now refers to a different process
    }
    // SAFETY: `kill` with an absolute pid sends the signal to exactly that
    // process; the pid came from /proc and is revalidated by the kernel.
    unsafe { libc::kill(pid as i32, libc::SIGKILL) == 0 }
}

#[cfg(target_os = "windows")]
fn kill_impl(pid: u32, expected_start_time: u64) -> bool {
    windows::kill(pid, expected_start_time)
}
