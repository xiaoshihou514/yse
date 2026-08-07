//! Windows backend: `windows-sys` FFI over Toolhelp / PSAPI / IP Helper /
//! the registry. Rate counters are diffed against the previous tick.

use crate::sys::{CpuInfo, NetRate, PowerLevel, ProcessGroup, ProcessSample, SystemStats};
use std::collections::HashMap;
use std::collections::HashSet;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, FILETIME, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetIfTable, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IFTABLE,
};
use windows_sys::Win32::Storage::FileSystem::GetLogicalDriveStringsW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{
    GetPerformanceInfo, GetProcessMemoryInfo, PERFORMANCE_INFORMATION, PROCESS_MEMORY_COUNTERS,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_DWORD, REG_SZ, RegCloseKey, RegOpenKeyExW,
    RegQueryValueExW,
};
use windows_sys::Win32::System::SystemInformation::{
    GetLogicalProcessorInformation, GetNativeSystemInfo, GetTickCount64, GlobalMemoryStatusEx,
    MEMORYSTATUSEX, RelationCache, RelationProcessorCore, SYSTEM_INFO,
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
};
use windows_sys::Win32::System::Threading::{
    GetProcessIoCounters, GetProcessTimes, GetSystemTimes, IO_COUNTERS, OpenProcess,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    TerminateProcess,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};
use windows_sys::core::BOOL;

fn filetime_to_u64(ft: &FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len])
}

struct SystemTimes {
    idle: u64,
    kernel: u64,
    user: u64,
}

fn system_times() -> Option<SystemTimes> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all pointers point to writable FILETIME values.
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
    ok.then(|| SystemTimes {
        idle: filetime_to_u64(&idle),
        kernel: filetime_to_u64(&kernel),
        user: filetime_to_u64(&user),
    })
}

struct ProcTick {
    ticks: u64,
    io_bytes: u64,
}

/// Returns `(working set bytes, io bytes, kernel+user ticks)`.
fn process_exe_path(handle: HANDLE) -> String {
    let mut size: u32 = 1024;
    let mut buffer = vec![0u16; size as usize];
    loop {
        // SAFETY: `buffer` is writable for `size` u16 slots; the API updates
        // `size` with the number of characters written.
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
        if ok != 0 {
            return wide_to_string(&buffer[..size as usize]);
        }
        if unsafe { GetLastError() } == ERROR_INSUFFICIENT_BUFFER {
            size = size.saturating_mul(2).max(1024);
            buffer.resize(size as usize, 0);
            continue;
        }
        return String::new();
    }
}

fn visible_window_pids() -> HashSet<u32> {
    let mut pids = HashSet::new();
    // SAFETY: the callback writes only into `pids`, whose address is passed as
    // LPARAM; the enumeration runs synchronously on this thread.
    unsafe extern "system" fn collect(window: HWND, lparam: isize) -> BOOL {
        if IsWindowVisible(window) != 0 {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(window, &mut pid);
            let pids = &mut *(lparam as *mut HashSet<u32>);
            pids.insert(pid);
        }
        1
    }
    unsafe {
        EnumWindows(Some(collect), &mut pids as *mut HashSet<u32> as isize);
    }
    pids
}

fn process_group(pid: u32, name: &str, windows: &HashSet<u32>) -> ProcessGroup {
    const SYSTEM_NAMES: &[&str] = &[
        "[System Process]",
        "System",
        "Registry",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "winlogon.exe",
        "services.exe",
        "lsass.exe",
        "svchost.exe",
        "fontdrvhost.exe",
        "dwm.exe",
        "conhost.exe",
    ];
    if SYSTEM_NAMES.contains(&name) {
        ProcessGroup::System
    } else if windows.contains(&pid) {
        ProcessGroup::App
    } else {
        ProcessGroup::Background
    }
}

fn priority_class(priority: i32) -> String {
    match priority {
        0x100 => String::from("实时"),
        0x80 => String::from("高"),
        0x8000 => String::from("高于正常"),
        0x20 => String::from("普通"),
        0x4000 => String::from("低于正常"),
        _ => String::from("低"),
    }
}

/// Returns `(working set bytes, io bytes, kernel+user ticks, exe path)`.
fn process_details(pid: u32) -> (u64, u64, u64, String) {
    // SAFETY: OpenProcess with a valid access mask and pid.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return (0, 0, 0, String::new());
    }
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let mut io = IO_COUNTERS::default();
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: each struct is sized and writable; handles were just opened.
    let mem = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0
    }
    .then_some(counters.WorkingSetSize as u64)
    .unwrap_or(0);
    let io_bytes = unsafe { GetProcessIoCounters(handle, &mut io) != 0 }
        .then_some(io.ReadTransferCount + io.WriteTransferCount)
        .unwrap_or(0);
    let ticks =
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 }
            .then_some(filetime_to_u64(&kernel) + filetime_to_u64(&user))
            .unwrap_or(0);
    let exe = process_exe_path(handle);
    // SAFETY: `handle` was returned by OpenProcess and is still open.
    unsafe { CloseHandle(handle) };
    (mem, io_bytes, ticks, exe)
}

fn process_table(
    prev: &mut HashMap<u32, ProcTick>,
    total_delta: u64,
    windows: &HashSet<u32>,
) -> Vec<ProcessSample> {
    let mut processes = Vec::new();
    // SAFETY: standard Toolhelp snapshot pattern; entries are initialized
    // before use and the snapshot handle is closed on every path.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return processes;
    }
    let mut entry = PROCESSENTRY32W::default();
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while ok {
        let pid = entry.th32ProcessID;
        let name = wide_to_string(&entry.szExeFile);
        let (mem, io_bytes, ticks, exe) = process_details(pid);
        let cpu = match prev.get(&pid) {
            Some(p) if total_delta > 0 => {
                ticks.saturating_sub(p.ticks) as f64 / total_delta as f64 * 100.0
            }
            _ => 0.0,
        };
        let disk = prev
            .get(&pid)
            .map(|p| io_bytes.saturating_sub(p.io_bytes))
            .unwrap_or(0);
        prev.insert(pid, ProcTick { ticks, io_bytes });
        let group = process_group(pid, &name, windows);
        processes.push(ProcessSample {
            pid,
            name,
            exe,
            cpu,
            mem_bytes: mem,
            disk_bytes_per_s: disk,
            net_bytes_per_s: 0,
            power: PowerLevel::from_cpu(cpu),
            parent_pid: entry.th32ParentProcessID,
            threads: entry.cntThreads,
            cpu_ticks: ticks,
            priority: priority_class(entry.pcPriClassBase),
            group,
        });
        ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    unsafe { CloseHandle(snapshot) };
    processes
}

fn memory_status() -> (u64, u64) {
    let mut status = MEMORYSTATUSEX::default();
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is sized and initialized.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return (0, 0);
    }
    (
        status.ullTotalPhys,
        status.ullTotalPhys.saturating_sub(status.ullAvailPhys),
    )
}

fn performance_counts() -> (u32, u32, u32) {
    let mut perf = PERFORMANCE_INFORMATION::default();
    perf.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    // SAFETY: `perf` is sized and initialized.
    if unsafe {
        GetPerformanceInfo(
            &mut perf,
            std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32,
        )
    } == 0
    {
        return (0, 0, 0);
    }
    (perf.ProcessCount, perf.ThreadCount, perf.HandleCount)
}

fn reg_query_string(subkey: &str, value: &str) -> Option<String> {
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: `wide(subkey)` is a NUL-terminated buffer valid for the call.
    if unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(subkey).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    } != 0
    {
        return None;
    }
    let mut buffer = [0u8; 1024];
    let mut size = buffer.len() as u32;
    let mut ty = 0u32;
    // SAFETY: `buffer` is writable for `size` bytes; key was opened above.
    let result = unsafe {
        RegQueryValueExW(
            key,
            wide(value).as_ptr(),
            std::ptr::null(),
            &mut ty,
            buffer.as_mut_ptr(),
            &mut size,
        )
    };
    // SAFETY: `key` was returned by RegOpenKeyExW.
    unsafe { RegCloseKey(key) };
    if result != 0 || ty != REG_SZ {
        return None;
    }
    let words =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, size as usize / 2) };
    Some(wide_to_string(words))
}

fn reg_query_dword(subkey: &str, value: &str) -> Option<u32> {
    let mut key: HKEY = std::ptr::null_mut();
    if unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            wide(subkey).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    } != 0
    {
        return None;
    }
    let mut buffer = [0u8; 4];
    let mut size = buffer.len() as u32;
    let mut ty = 0u32;
    let result = unsafe {
        RegQueryValueExW(
            key,
            wide(value).as_ptr(),
            std::ptr::null(),
            &mut ty,
            buffer.as_mut_ptr(),
            &mut size,
        )
    };
    unsafe { RegCloseKey(key) };
    if result != 0 || ty != REG_DWORD {
        return None;
    }
    Some(u32::from_ne_bytes(buffer))
}

fn cpu_identity() -> (String, f64, f64, u32, u32, u32, u64, u64, u64) {
    const CPU_KEY: &str = "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0";
    let model =
        reg_query_string(CPU_KEY, "ProcessorNameString").unwrap_or_else(|| "Unknown CPU".into());
    let mhz = reg_query_dword(CPU_KEY, "~MHz").unwrap_or(0) as f64;

    let mut sockets = 0u32;
    for index in 0..8 {
        let key = format!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\{index}");
        let mut handle: HKEY = std::ptr::null_mut();
        if unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide(&key).as_ptr(),
                0,
                KEY_READ,
                &mut handle,
            )
        } != 0
        {
            break;
        }
        unsafe { RegCloseKey(handle) };
        sockets += 1;
    }
    sockets = sockets.max(1);

    let mut info = SYSTEM_INFO::default();
    // SAFETY: `info` is sized.
    unsafe { GetNativeSystemInfo(&mut info) };
    let logical = info.dwNumberOfProcessors.max(1);

    let (cores, l1_kb, l2_kb, l3_kb) = logical_processor_caches();
    (
        model, mhz, mhz, sockets, cores, logical, l1_kb, l2_kb, l3_kb,
    )
}

fn logical_processor_caches() -> (u32, u64, u64, u64) {
    let mut needed: u32 = 0;
    // SAFETY: first call only writes `needed`; a null buffer is expected and
    // reports the required size via ERROR_INSUFFICIENT_BUFFER.
    unsafe {
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut needed);
    }
    if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || needed == 0 {
        return (1, 0, 0, 0);
    }
    let count = needed as usize / std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
    let mut buffer = vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); count];
    // SAFETY: `buffer` is exactly `needed` bytes and writable.
    if unsafe { GetLogicalProcessorInformation(buffer.as_mut_ptr(), &mut needed) } == 0 {
        return (1, 0, 0, 0);
    }
    let mut cores = 0u32;
    let mut l1 = 0u64;
    let mut l2 = 0u64;
    let mut l3 = 0u64;
    for info in &buffer {
        if info.Relationship == RelationProcessorCore {
            cores += 1;
        } else if info.Relationship == RelationCache {
            // SAFETY: the union is active as Cache only when the relationship
            // is RelationCache, which is what we just checked.
            let cache = unsafe { info.Anonymous.Cache };
            let kb = cache.Size as u64 / 1024;
            match cache.Level {
                1 => l1 = l1.max(kb),
                2 => l2 = l2.max(kb),
                3 => l3 = l3.max(kb),
                _ => {}
            }
        }
    }
    (cores.max(1), l1, l2, l3)
}

#[cfg(target_arch = "x86_64")]
fn virtualization_enabled() -> bool {
    // SAFETY: `__cpuid(1)` is a stable x86_64 intrinsic.
    let leaf = std::arch::x86_64::__cpuid(1);
    // ECX bit 31 = hypervisor present, bit 5 = VMX, bit 2 = SVM.
    (leaf.ecx & ((1 << 31) | (1 << 5) | (1 << 2))) != 0
}

#[cfg(not(target_arch = "x86_64"))]
fn virtualization_enabled() -> bool {
    false
}

fn net_rates(prev: &mut HashMap<u32, (u32, u32)>) -> Vec<NetRate> {
    let mut size: u32 = 0;
    // SAFETY: first call reports the required buffer size.
    unsafe {
        GetIfTable(std::ptr::null_mut(), &mut size, 0);
    }
    if size == 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; size as usize];
    // SAFETY: `bytes` is exactly `size` bytes and is writable as MIB_IFTABLE.
    if unsafe { GetIfTable(bytes.as_mut_ptr() as *mut MIB_IFTABLE, &mut size, 0) } != 0 {
        return Vec::new();
    }
    // SAFETY: the buffer now holds a valid MIB_IFTABLE with dwNumEntries rows.
    let table = unsafe { &*(bytes.as_ptr() as *const MIB_IFTABLE) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
    let mut rates = Vec::new();
    for row in rows {
        if row.dwType == IF_TYPE_SOFTWARE_LOOPBACK {
            continue;
        }
        if let Some((prev_rx, prev_tx)) = prev.get(&row.dwIndex) {
            rates.push(NetRate {
                name: wide_to_string(&row.wszName),
                rx_bps: row.dwInOctets.saturating_sub(*prev_rx) as u64,
                tx_bps: row.dwOutOctets.saturating_sub(*prev_tx) as u64,
            });
        }
        prev.insert(row.dwIndex, (row.dwInOctets, row.dwOutOctets));
    }
    rates
}

fn disk_names() -> Vec<String> {
    let mut buffer = vec![0u16; 256];
    // SAFETY: `buffer` is writable for its length.
    let len = unsafe { GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr()) };
    if len == 0 {
        return Vec::new();
    }
    buffer[..len as usize]
        .split(|&c| c == 0)
        .filter(|drive| !drive.is_empty())
        .map(wide_to_string)
        .collect()
}

fn gpu_name() -> String {
    const GPU_KEY: &str =
        "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}\\0000";
    reg_query_string(GPU_KEY, "DriverDesc").unwrap_or_else(|| "—".into())
}

/// Windows-specific sampler holding previous counters for rate diffs.
pub struct WindowsSampler {
    prev_system: Option<SystemTimes>,
    prev_proc: HashMap<u32, ProcTick>,
    prev_nets: HashMap<u32, (u32, u32)>,
    identity: (String, f64, f64, u32, u32, u32, u64, u64, u64),
    virtualization: bool,
    gpu: String,
}

impl WindowsSampler {
    pub fn new() -> Self {
        Self {
            prev_system: None,
            prev_proc: HashMap::new(),
            prev_nets: HashMap::new(),
            identity: cpu_identity(),
            virtualization: virtualization_enabled(),
            gpu: gpu_name(),
        }
    }

    pub fn sample(&mut self) -> SystemStats {
        let windows = visible_window_pids();
        let current = system_times();
        let (cpu_pct, total_delta) = match (&self.prev_system, &current) {
            (Some(prev), Some(cur)) => {
                let d_total = (cur.kernel + cur.user).saturating_sub(prev.kernel + prev.user);
                let d_idle = cur.idle.saturating_sub(prev.idle);
                let pct = if d_total > 0 {
                    (d_total - d_idle) as f64 / d_total as f64 * 100.0
                } else {
                    0.0
                };
                (pct, d_total)
            }
            _ => (0.0, 0),
        };
        self.prev_system = current;
        let processes = process_table(&mut self.prev_proc, total_delta, &windows);
        let (mem_total, mem_used) = memory_status();
        let (process_count, thread_count, handle_count) = performance_counts();
        let nets = net_rates(&mut self.prev_nets);
        let (model, current_mhz, base_mhz, sockets, cores, logical, l1_kb, l2_kb, l3_kb) =
            &self.identity;
        let _ = model;
        SystemStats {
            processes,
            cpu: CpuInfo {
                usage_pct: cpu_pct,
                per_core_pct: Vec::new(),
                current_mhz: *current_mhz,
                base_mhz: *base_mhz,
                sockets: *sockets,
                cores: *cores,
                logical: *logical,
                virtualization: self.virtualization,
                l1_kb: *l1_kb,
                l2_kb: *l2_kb,
                l3_kb: *l3_kb,
            },
            mem_total,
            mem_used,
            uptime_secs: unsafe { GetTickCount64() } / 1000,
            process_count,
            thread_count,
            handle_count,
            nets,
            disk_names: disk_names(),
            gpu_name: self.gpu.clone(),
        }
    }
}

pub fn kill(pid: u32) -> bool {
    // SAFETY: PROCESS_TERMINATE is the required access for TerminateProcess.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let ok = unsafe { TerminateProcess(handle, 1) } != 0;
    unsafe { CloseHandle(handle) };
    ok
}
