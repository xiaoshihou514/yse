//! Linux backend: pure-`std` readers over `/proc` and `/sys`.

use crate::sys::{CpuInfo, NetRate, PowerLevel, ProcessGroup, ProcessSample, SystemStats};
use std::collections::HashMap;

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn parse_u64(text: &str) -> u64 {
    text.trim().parse().unwrap_or(0)
}

/// All ticks of the CPU lines in /proc/stat: `(name, total, idle)`.
fn cpu_ticks() -> Vec<(String, u64, u64)> {
    let mut lines = Vec::new();
    if let Some(stat) = read("/proc/stat") {
        for line in stat.lines().filter(|line| line.starts_with("cpu")) {
            let mut fields = line.split_whitespace();
            let name = fields.next().unwrap_or_default().to_string();
            let values: Vec<u64> = fields.filter_map(|f| f.parse().ok()).collect();
            if values.is_empty() {
                continue;
            }
            let total: u64 = values.iter().sum();
            let idle: u64 =
                values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
            lines.push((name, total, idle));
        }
    }
    lines
}

fn meminfo() -> (u64, u64) {
    let mut total = 0u64;
    let mut available = 0u64;
    if let Some(info) = read("/proc/meminfo") {
        for line in info.lines() {
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("MemTotal:") => total = parse_u64(parts.next().unwrap_or("0")) * 1024,
                Some("MemAvailable:") => available = parse_u64(parts.next().unwrap_or("0")) * 1024,
                _ => {}
            }
        }
    }
    (total, total.saturating_sub(available))
}

fn uptime_secs() -> u64 {
    read("/proc/uptime")
        .and_then(|text| text.split_whitespace().next().map(str::to_string))
        .and_then(|secs| secs.parse::<f64>().ok())
        .map(|secs| secs as u64)
        .unwrap_or(0)
}

struct StatTick {
    proc_ticks: u64,
    io_bytes: u64,
}

struct ProcStat {
    name: String,
    parent_pid: u32,
    ticks: u64,
    nice: i64,
}

fn parse_proc_stat(stat: &str) -> Option<ProcStat> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let name = stat[open + 1..close].to_string();
    let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    // rest[0] is field 3 (state); field 4 = ppid, field 14 = utime,
    // field 15 = stime, field 19 = nice.
    let parent_pid: u32 = rest.get(1)?.parse().ok()?;
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    let nice: i64 = rest.get(16)?.parse().ok()?;
    Some(ProcStat {
        name,
        parent_pid,
        ticks: utime + stime,
        nice,
    })
}

fn process_io_bytes(pid: u32) -> u64 {
    read(&format!("/proc/{pid}/io"))
        .map(|io| {
            let mut bytes = 0u64;
            for line in io.lines() {
                let mut parts = line.split_whitespace();
                match parts.next() {
                    Some("read_bytes:") | Some("write_bytes:") => {
                        bytes += parse_u64(parts.next().unwrap_or("0"));
                    }
                    _ => {}
                }
            }
            bytes
        })
        .unwrap_or(0)
}

fn process_exe(pid: u32, name: &str) -> String {
    if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/exe")) {
        let path = path.to_string_lossy().into_owned();
        if !path.is_empty() {
            return path;
        }
    }
    // `/proc/<pid>/exe` can be unreadable (hidepid / ptrace restrictions);
    // fall back to a PATH lookup so the process list still gets icons.
    if !name.contains('/')
        && let Some(path) = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let candidate = dir.join(name);
                candidate
                    .is_file()
                    .then(|| candidate.to_string_lossy().into_owned())
            })
        })
    {
        return path;
    }
    String::new()
}

fn process_group(pid: u32, name: &str) -> ProcessGroup {
    const SYSTEM_NAMES: &[&str] = &[
        "systemd",
        "systemd-journald",
        "systemd-logind",
        "systemd-udevd",
        "systemd-resolved",
        "systemd-timesyncd",
        "systemd-networkd",
        "dbus-daemon",
        "dbus-broker",
        "cron",
        "atd",
        "sshd",
        "polkitd",
        "NetworkManager",
        "agetty",
        "login",
        "init",
        "kthreadd",
        "ksoftirqd",
        "kworker",
        "rcu",
        "irq",
        "watchdog",
        "migration",
        "cpuhp",
        "kcompactd",
        "khugepaged",
        "kswapd",
        "kdevtmpfs",
        "bdi-default",
        "jbd2",
        "xfs",
        "ext4",
    ];
    if name.starts_with('[') || SYSTEM_NAMES.contains(&name) {
        return ProcessGroup::System;
    }
    // A process that inherited a display connection is a user application.
    if read(&format!("/proc/{pid}/environ"))
        .is_some_and(|environ| environ.contains("WAYLAND_DISPLAY=") || environ.contains("DISPLAY="))
    {
        return ProcessGroup::App;
    }
    ProcessGroup::Background
}

struct ProcFields {
    name: String,
    parent_pid: u32,
    rss_bytes: u64,
    threads: u32,
    ticks: u64,
    priority: String,
}

fn process_fields(pid: u32) -> Option<ProcFields> {
    let stat = read(&format!("/proc/{pid}/stat"))?;
    let parsed = parse_proc_stat(&stat)?;
    let priority = match parsed.nice {
        nice if nice < 0 => String::from("高"),
        0 => String::from("普通"),
        _ => String::from("低"),
    };
    let status = read(&format!("/proc/{pid}/status")).unwrap_or_default();
    let mut rss_bytes = 0u64;
    let mut threads = 0u32;
    for line in status.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("VmRSS:") => rss_bytes = parse_u64(parts.next().unwrap_or("0")) * 1024,
            Some("Threads:") => threads = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            _ => {}
        }
    }
    Some(ProcFields {
        name: parsed.name,
        parent_pid: parsed.parent_pid,
        rss_bytes,
        threads,
        ticks: parsed.ticks,
        priority,
    })
}

fn net_rates(prev: &mut HashMap<String, (u64, u64)>) -> Vec<NetRate> {
    let mut rates = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return rates;
    };
    let now: HashMap<String, (u64, u64)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| {
            let rx = read(&format!("/sys/class/net/{name}/statistics/rx_bytes"))
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let tx = read(&format!("/sys/class/net/{name}/statistics/tx_bytes"))
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0);
            (name, (rx, tx))
        })
        .collect();
    for (name, (rx, tx)) in &now {
        if let Some((prev_rx, prev_tx)) = prev.get(name) {
            rates.push(NetRate {
                name: name.clone(),
                rx_bps: rx.saturating_sub(*prev_rx),
                tx_bps: tx.saturating_sub(*prev_tx),
            });
        }
        prev.insert(name.clone(), (*rx, *tx));
    }
    rates
}

fn disk_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().into_string().unwrap_or_default();
            if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
                continue;
            }
            names.push(name);
        }
    }
    names.sort();
    names
}

fn gpu_name() -> String {
    let vendor = read("/sys/class/drm/card0/device/vendor")
        .unwrap_or_default()
        .trim()
        .to_string();
    match vendor.as_str() {
        "0x8086" => "Intel UHD".to_string(),
        "0x1002" => "AMD Radeon".to_string(),
        "0x10de" => "NVIDIA GeForce".to_string(),
        "0x1af4" => "Virtio GPU".to_string(),
        other => {
            if other.is_empty() {
                "—".to_string()
            } else {
                format!("GPU {other}")
            }
        }
    }
}

fn cpuinfo() -> (String, f64, f64, u32, u32, u32, bool) {
    let text = read("/proc/cpuinfo").unwrap_or_default();
    let mut model = String::new();
    let mut current_mhz = 0.0f64;
    let mut logical = 0u32;
    let mut sockets = std::collections::HashSet::new();
    let mut core_ids = std::collections::HashSet::new();
    let mut virtualization = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "model name" if model.is_empty() => model = value.to_string(),
            "cpu MHz" if current_mhz == 0.0 => current_mhz = value.parse().unwrap_or(0.0),
            "processor" => logical += 1,
            "physical id" => {
                sockets.insert(value.to_string());
            }
            "core id" => {
                core_ids.insert(value.to_string());
            }
            "flags" if value.contains("vmx") || value.contains("svm") => virtualization = true,
            _ => {}
        }
    }
    let sockets = sockets.len().max(1) as u32;
    let cores = core_ids.len().max(1) as u32;
    (
        model,
        current_mhz,
        current_mhz,
        sockets,
        cores,
        logical,
        virtualization,
    )
}

fn cache_kb() -> (u64, u64, u64) {
    let mut l1 = 0u64;
    let mut l2 = 0u64;
    let mut l3 = 0u64;
    for index in 0..8 {
        let base = format!("/sys/devices/system/cpu/cpu0/cache/index{index}");
        let (Some(level), Some(size)) = (
            read(&format!("{base}/level")).and_then(|v| v.trim().parse::<u32>().ok()),
            read(&format!("{base}/size")),
        ) else {
            break;
        };
        let size = size.trim().to_ascii_uppercase();
        let kb = if let Some(value) = size.strip_suffix('K') {
            value.parse::<u64>().unwrap_or(0)
        } else if let Some(value) = size.strip_suffix('M') {
            value.parse::<u64>().unwrap_or(0) * 1024
        } else {
            0
        };
        match level {
            1 => l1 += kb,
            2 => l2 = l2.max(kb),
            3 => l3 = l3.max(kb),
            _ => {}
        }
    }
    (l1, l2, l3)
}

/// Linux-specific sampler holding the previous tick so rates can be computed.
pub struct LinuxSampler {
    prev_cpu: Option<Vec<(String, u64, u64)>>,
    prev_total_delta: u64,
    prev_proc: HashMap<u32, StatTick>,
    prev_nets: HashMap<String, (u64, u64)>,
}

impl LinuxSampler {
    pub fn new() -> Self {
        Self {
            prev_cpu: None,
            prev_total_delta: 0,
            prev_proc: HashMap::new(),
            prev_nets: HashMap::new(),
        }
    }

    pub fn sample(&mut self) -> SystemStats {
        let current_cpu = cpu_ticks();
        let mut usage_pct = 0.0f64;
        let mut per_core_pct = Vec::new();
        let mut total_delta = 0u64;
        if let Some(prev) = &self.prev_cpu {
            for (name, total, idle) in &current_cpu {
                if let Some((_, prev_total, prev_idle)) = prev.iter().find(|(n, _, _)| n == name) {
                    let dt = total.saturating_sub(*prev_total);
                    let di = idle.saturating_sub(*prev_idle);
                    if name == "cpu" {
                        total_delta = dt;
                    }
                    let pct = if dt > 0 {
                        (dt - di) as f64 / dt as f64 * 100.0
                    } else {
                        0.0
                    };
                    if name == "cpu" {
                        usage_pct = pct;
                    } else {
                        per_core_pct.push(pct);
                    }
                }
            }
        }
        self.prev_cpu = Some(current_cpu);

        let total_ticks_delta = self.prev_total_delta;
        self.prev_total_delta = total_delta;
        let mut processes = Vec::new();
        let mut thread_count = 0u32;
        let mut process_count = 0u32;
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                let Ok(pid) = name.parse::<u32>() else {
                    continue;
                };
                let Some(fields) = process_fields(pid) else {
                    continue;
                };
                let stat_ticks = fields.ticks;
                let io_bytes = process_io_bytes(pid);
                let (cpu, disk_bps) = match self.prev_proc.get(&pid) {
                    Some(prev) if total_ticks_delta > 0 => {
                        let cpu = (stat_ticks.saturating_sub(prev.proc_ticks)) as f64
                            / total_ticks_delta as f64
                            * 100.0;
                        let disk = io_bytes.saturating_sub(prev.io_bytes);
                        (cpu, disk)
                    }
                    _ => (0.0, 0),
                };
                self.prev_proc.insert(
                    pid,
                    StatTick {
                        proc_ticks: stat_ticks,
                        io_bytes,
                    },
                );
                thread_count += fields.threads;
                process_count += 1;
                let name = fields.name;
                let group = process_group(pid, &name);
                processes.push(ProcessSample {
                    pid,
                    parent_pid: fields.parent_pid,
                    exe: process_exe(pid, &name),
                    name,
                    threads: fields.threads,
                    cpu_ticks: stat_ticks,
                    priority: fields.priority,
                    group,
                    cpu,
                    mem_bytes: fields.rss_bytes,
                    disk_bytes_per_s: disk_bps,
                    net_bytes_per_s: 0,
                    power: PowerLevel::from_cpu(cpu),
                });
            }
        }

        let (mem_total, mem_used) = meminfo();
        let (_model, current_mhz, base_mhz, sockets, cores, logical, virtualization) = cpuinfo();
        let (l1_kb, l2_kb, l3_kb) = cache_kb();
        let nets = net_rates(&mut self.prev_nets);
        let uptime = uptime_secs();

        SystemStats {
            processes,
            cpu: CpuInfo {
                usage_pct,
                per_core_pct,
                current_mhz,
                base_mhz,
                sockets,
                cores,
                logical,
                virtualization,
                l1_kb,
                l2_kb,
                l3_kb,
            },
            mem_total,
            mem_used,
            uptime_secs: uptime,
            process_count,
            thread_count,
            handle_count: 0,
            nets,
            disk_names: disk_names(),
            gpu_name: gpu_name(),
        }
    }
}
