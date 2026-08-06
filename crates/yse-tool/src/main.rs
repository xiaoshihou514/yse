//! `cargo yse` — the Yse developer toolchain.
//!
//! Install with `cargo install --path crates/yse-tool`, then:
//!
//! - `cargo yse new <name>` — generate a new Yse desktop application.
//! - `cargo yse dev` — build and run the current project.
//! - `cargo yse test` — run the project's tests.
//! - `cargo yse bundle` — build a release and produce a platform bundle.

mod project;
mod template;

use std::env;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    let result = match command {
        "new" => command_new(&args[1..]),
        "dev" => command_dev(&args[1..]),
        "test" => command_test(&args[1..]),
        "bundle" => command_bundle(&args[1..]),
        "version" | "--version" => {
            println!("cargo-yse {VERSION}");
            Ok(())
        }
        "help" | "--help" | "" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    };

    if let Err(message) = result {
        eprintln!("error: {message}");
        eprintln!("Run `cargo yse --help` for usage.");
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "cargo-yse {VERSION} — Yse developer toolchain\n\
         \n\
         USAGE:\n\
         \x20   cargo yse new <name>       Generate a new Yse desktop application\n\
         \x20   cargo yse new --local <yse-path> <name>\n\
         \x20                              Generate against a local Yse checkout (facade)\n\
         \x20   cargo yse dev              Build and run the current project\n\
         \x20   cargo yse test             Run the current project's tests\n\
         \x20   cargo yse bundle           Build a release and produce a platform bundle\n\
         \x20   cargo yse version          Print the version\n"
    );
}

fn command_new(args: &[String]) -> Result<(), String> {
    let (name, local_path) = if args.first().map(String::as_str) == Some("--local") {
        if args.len() < 3 {
            return Err("`new --local <yse-path> <name>` requires a path and a name".into());
        }
        (args[2].clone(), Some(args[1].clone()))
    } else {
        let Some(name) = args.first() else {
            return Err("`new` requires a project name, e.g. `cargo yse new hello`".into());
        };
        (name.clone(), None)
    };
    let project = match &local_path {
        Some(path) => project::Project::parse_local(&name, path)?,
        None => project::Project::parse(&name)?,
    };
    let target = env::current_dir()
        .map_err(|error| format!("cannot read current directory: {error}"))?
        .join(&project.name);
    if target.exists() {
        return Err(format!("directory `{}` already exists", target.display()));
    }
    if local_path.is_some() {
        project::write_project_local(&project, &target)?;
    } else {
        project::write_project(&project, &target)?;
    }
    println!(
        "Generated {} at {}\n\
         \n\
         Next steps:\n\
         \x20   cd {}\n\
         \x20   cargo yse dev       # build and run (requires Qt 6 development files)\n\
         \x20   cargo yse bundle    # build a release bundle",
        project.name,
        target.display(),
        project.name
    );
    Ok(())
}

fn command_dev(args: &[String]) -> Result<(), String> {
    check_qt()?;
    let mut cargo_args = vec!["run".to_string()];
    cargo_args.extend_from_slice(args);
    run_cargo(&cargo_args)
}

fn command_test(_args: &[String]) -> Result<(), String> {
    check_qt()?;
    run_cargo(&["test".to_string()])
}

fn command_bundle(_args: &[String]) -> Result<(), String> {
    check_qt()?;
    let project = project::Project::from_manifest("yse.toml")?;
    run_cargo(&["build".to_string(), "--release".to_string()])?;
    project::bundle(&project)
}

fn run_cargo(args: &[String]) -> Result<(), String> {
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|error| format!("failed to run cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

/// Cheap pre-flight check that a Qt 6 development installation is visible.
fn check_qt() -> Result<(), String> {
    let found = Command::new("pkg-config")
        .args(["--modversion", "Qt6Widgets"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if found {
        return Ok(());
    }
    let has_qmake = ["qmake6", "qmake"].iter().any(|tool| {
        Command::new(tool)
            .arg("-v")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    });
    if has_qmake {
        return Ok(());
    }
    Err("Qt 6 development files not found.\n\
         On Debian/Ubuntu: sudo apt install qt6-base-dev ninja-build libgl1-mesa-dev\n\
         On Windows/macOS: install Qt 6 (e.g. via aqtinstall) and put qmake on PATH"
        .into())
}
