//! `gansi doctor` — environment health report, in the spirit of
//! `flutter doctor`: each check prints a status line, and the command exits
//! non-zero when any required check fails.

use std::path::Path;
use std::process::Command;

use crate::qt;

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Warn,
    Fail,
}

struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

pub fn run(base: &Path) -> Result<(), String> {
    let mut checks = Vec::new();

    // Rust toolchain.
    let cargo = Command::new("cargo")
        .arg("--version")
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false);
    if cargo {
        checks.push(Check {
            name: "Rust toolchain",
            status: Status::Ok,
            detail: Command::new("cargo")
                .arg("--version")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "cargo".into()),
        });
    } else {
        checks.push(Check {
            name: "Rust toolchain",
            status: Status::Fail,
            detail: "cargo not found on PATH; install via rustup".into(),
        });
    }

    // Qt 6.
    match qt::qt_prefix(base) {
        Some(prefix) => checks.push(Check {
            name: "Qt 6",
            status: Status::Ok,
            detail: prefix.display().to_string(),
        }),
        None => checks.push(Check {
            name: "Qt 6",
            status: Status::Fail,
            detail: "not found; run `gansi setup` to install it automatically".into(),
        }),
    }

    // Build essentials: CMake is required by cxx-qt-build; Ninja on Linux.
    let cmake = tool_version("cmake");
    if let Some(version) = cmake {
        checks.push(Check {
            name: "CMake",
            status: Status::Ok,
            detail: version,
        });
    } else {
        checks.push(Check {
            name: "CMake",
            status: Status::Fail,
            detail: "required by cxx-qt-build; install cmake".into(),
        });
    }
    #[cfg(target_os = "linux")]
    {
        let ninja = tool_version("ninja");
        if let Some(version) = ninja {
            checks.push(Check {
                name: "Ninja",
                status: Status::Ok,
                detail: version,
            });
        } else {
            checks.push(Check {
                name: "Ninja",
                status: Status::Warn,
                detail: "optional; CMake may fall back to another generator".into(),
            });
        }
    }

    // Project state.
    if Path::new("yse.toml").exists() {
        checks.push(Check {
            name: "Project",
            status: Status::Ok,
            detail: "yse.toml present (created by `gansi new`)".into(),
        });
    } else {
        checks.push(Check {
            name: "Project",
            status: Status::Warn,
            detail: "no yse.toml in the current directory; `gansi new` creates one".into(),
        });
    }

    // Report.
    let mut failed = false;
    let mut warned = false;
    for check in &checks {
        let mark = match check.status {
            Status::Ok => "[✓]",
            Status::Warn => "[!]",
            Status::Fail => "[✗]",
        };
        println!("{mark} {}: {}", check.name, check.detail);
        if check.status == Status::Fail {
            failed = true;
        }
        if check.status == Status::Warn {
            warned = true;
        }
    }
    println!();
    if failed {
        Err("some required checks failed; run `gansi setup` and retry".into())
    } else if warned {
        println!("doctor: all required checks passed (warnings above are optional).");
        Ok(())
    } else {
        println!("doctor: everything looks good!");
        Ok(())
    }
}

fn tool_version(tool: &str) -> Option<String> {
    let output = Command::new(tool).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or(tool)
            .trim()
            .to_string(),
    )
}
