//! Self-contained Qt 6 acquisition.
//!
//! `gansi` should not require the user to install Qt by hand. This module
//! detects an existing Qt 6 development installation, and when none is found
//! downloads prebuilt Qt 6 binaries with `aqt` (installed through `uv`) into
//! the project's `.gansi/qt` — the same flow on every platform.
//!
//! The discovered Qt prefix is recorded in `.gansi/config.toml` so
//! `gansi dev/test/bundle` can point the build at it without any manual
//! environment setup.

#[allow(unused_imports)] // `env` is used only in the unsupported-OS fallback.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Preferred Qt 6 version used by the `aqt` download.
const QT_VERSION: &str = "6.8.3";
/// Modules needed by a Yse Widgets application.
const QT_MODULES: &str = "qtbase";

const CONFIG_DIR: &str = ".gansi";
const CONFIG_FILE: &str = "config.toml";

/// Where `gansi` keeps its per-project state (Qt prefix, flags, ...).
pub fn state_dir(base: &Path) -> PathBuf {
    base.join(CONFIG_DIR)
}

/// Find an existing Qt 6 development installation on `PATH` / via pkg-config.
pub fn find_system_qt() -> Option<PathBuf> {
    // pkg-config knows the canonical Qt 6 prefix on Linux.
    let output = Command::new("pkg-config")
        .args(["--variable=prefix", "Qt6Widgets"])
        .output()
        .ok()?;
    if output.status.success() {
        let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !prefix.is_empty() {
            return Some(PathBuf::from(prefix));
        }
    }
    // `qmake -query QT_INSTALL_PREFIX` works on every platform when Qt is on
    // PATH (Windows aqt layout included).
    for qmake in ["qmake6", "qmake"] {
        let output = Command::new(qmake)
            .args(["-query", "QT_INSTALL_PREFIX"])
            .output()
            .ok()?;
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                return Some(PathBuf::from(prefix));
            }
        }
    }
    None
}

/// Resolve the Qt prefix for `base`: a recorded install wins, then a system
/// Qt, then a fresh install in `.gansi/qt`.
pub fn qt_prefix(base: &Path) -> Option<PathBuf> {
    if let Some(prefix) = read_recorded_prefix(base) {
        return Some(prefix);
    }
    if let Some(prefix) = find_system_qt() {
        return Some(prefix);
    }
    let local = state_dir(base).join("qt");
    if local.join("bin").join(qmake_name()).exists() {
        return Some(local);
    }
    None
}

/// Ensure Qt 6 is available, installing it when necessary. Returns the Qt
/// prefix that should be used for the build.
pub fn ensure_qt(base: &Path) -> Result<PathBuf, String> {
    if let Some(prefix) = qt_prefix(base) {
        return Ok(prefix);
    }
    println!("No Qt 6 development installation found; installing one for you.");
    let prefix = install_qt(base)?;
    record_prefix(base, &prefix)?;
    Ok(prefix)
}

fn qmake_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "qmake.exe"
    } else {
        "qmake"
    }
}

/// Install Qt 6 using the platform's native flow.
fn install_qt(base: &Path) -> Result<PathBuf, String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        install_qt_with_aqt(base)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err(format!(
            "automatic Qt 6 installation is not supported on {}; install Qt 6 \
             manually and put qmake on PATH",
            env::consts::OS
        ))
    }
}

/// Install prebuilt Qt 6 binaries with `aqt` into `.gansi/qt` — the same
/// flow on every platform, no admin rights and no manual installer.
fn install_qt_with_aqt(base: &Path) -> Result<PathBuf, String> {
    let tool = ensure_uv_tool("aqtinstall", "aqt")?;
    let dest = state_dir(base).join("qt");
    let (host, arch) = aqt_host_and_arch();
    let version_dir = dest.join(QT_VERSION).join(arch);
    if version_dir.join("bin").join(qmake_name()).exists() {
        // Already downloaded by an earlier interrupted run; skip re-download.
        return Ok(version_dir);
    }
    let output = Command::new(&tool)
        .args([
            "install-qt",
            host,
            "desktop",
            QT_VERSION,
            arch,
            "-m",
            QT_MODULES,
            "-O",
        ])
        .arg(&dest)
        .output()
        .map_err(|error| format!("failed to run aqt: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "aqt install-qt failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(version_dir)
}

/// The aqt `install-qt` host/arch pair for the current platform and CPU.
fn aqt_host_and_arch() -> (&'static str, &'static str) {
    #[cfg(target_os = "windows")]
    {
        let arm64 = env::var("PROCESSOR_ARCHITECTURE").as_deref() == Ok("ARM64");
        (
            "windows",
            if arm64 {
                "win64_arm64"
            } else {
                "win64_msvc2022_64"
            },
        )
    }
    #[cfg(target_os = "macos")]
    {
        (
            "mac",
            if env::consts::ARCH == "aarch64" {
                "macos_arm64"
            } else {
                "macos_x86_64"
            },
        )
    }
    #[cfg(target_os = "linux")]
    {
        (
            "linux",
            if env::consts::ARCH == "x86_64" {
                "linux_gcc_64"
            } else if env::consts::ARCH == "aarch64" {
                "linux_arm64"
            } else {
                "linux_gcc_64"
            },
        )
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        ("", "")
    }
}

/// Install a `uv` tool if it is missing, returning the executable name.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn ensure_uv_tool(tool_package: &str, tool_name: &str) -> Result<String, String> {
    let visible = Command::new(tool_name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if visible {
        return Ok(tool_name.to_string());
    }
    let status = Command::new("uv")
        .args(["tool", "install", tool_package])
        .status()
        .map_err(|error| format!("cannot run `uv` (needed to install {tool_package}): {error}"))?;
    if !status.success() {
        return Err(format!("`uv tool install {tool_package}` failed"));
    }
    Ok(tool_name.to_string())
}

/// Record the Qt prefix in `.gansi/config.toml`.
fn record_prefix(base: &Path, prefix: &Path) -> Result<(), String> {
    let dir = state_dir(base);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    let content = format!(
        "# Gansi project state.\n[qt]\nprefix = \"{}\"\n",
        prefix.to_string_lossy().replace('\\', "/")
    );
    fs::write(dir.join(CONFIG_FILE), content)
        .map_err(|error| format!("cannot write {}: {error}", dir.join(CONFIG_FILE).display()))
}

fn read_recorded_prefix(base: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(state_dir(base).join(CONFIG_FILE)).ok()?;
    let prefix = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("prefix = "))?
        .trim_matches('"')
        .to_string();
    if prefix.is_empty() {
        None
    } else {
        Some(PathBuf::from(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        std::env::temp_dir().join(format!("gansi-qt-test-{}", std::process::id()))
    }

    #[test]
    fn recorded_prefix_round_trips() {
        let base = scratch();
        let prefix = base.join("local").join("qt").join("6.8.3");
        record_prefix(&base, &prefix).unwrap();
        assert_eq!(
            read_recorded_prefix(&base),
            Some(prefix.clone()),
            "state file must preserve the installed Qt prefix"
        );
        assert_eq!(
            qt_prefix(&base),
            Some(prefix),
            "a recorded install wins over system lookup"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn aqt_host_is_linux() {
        let (host, arch) = aqt_host_and_arch();
        assert_eq!(host, "linux");
        assert!(
            arch == "linux_gcc_64" || arch == "linux_arm64",
            "unexpected aqt arch: {arch}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn windows_aqt_layout_is_resolvable() {
        // aqt install-qt -O <dest> writes <dest>/<version>/<arch>; the
        // recorded prefix must point at the arch directory, not the root.
        let base = scratch();
        let arch = "win64_msvc2022_64";
        let installed = base.join(".gansi").join("qt").join(QT_VERSION).join(arch);
        record_prefix(&base, &installed).unwrap();
        let prefix = qt_prefix(&base).expect("recorded prefix resolves");
        assert!(
            prefix.ends_with(arch),
            "prefix should be the arch directory: {}",
            prefix.display()
        );
    }

    #[test]
    fn empty_state_means_no_recorded_qt() {
        let base = scratch().join("nested");
        assert_eq!(read_recorded_prefix(&base), None);
        assert_eq!(state_dir(&base), base.join(".gansi"));
    }
}
