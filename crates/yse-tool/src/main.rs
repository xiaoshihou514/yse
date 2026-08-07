//! `gansi` — the Yse developer toolchain.
//!
//! Install with `cargo install --path crates/yse-tool`, then:
//!
//! - `gansi new <name>` — generate a new Yse desktop application.
//! - `gansi setup` — install Qt 6 automatically (no manual setup).
//! - `gansi dev` — build and run the current project.
//! - `gansi test` — run the project's tests.
//! - `gansi bundle` — build a release and produce a platform bundle.

mod project;
mod qt;
mod template;

use std::env;
use std::path::Path;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    let result = match command {
        "new" => command_new(&args[1..]),
        "setup" => command_setup(&args[1..]),
        "dev" => command_dev(&args[1..]),
        "test" => command_test(&args[1..]),
        "bundle" => command_bundle(&args[1..]),
        "version" | "--version" => {
            println!("gansi {VERSION}");
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
        eprintln!("Run `gansi --help` for usage.");
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "gansi {VERSION} — Yse developer toolchain\n\
         \n\
         USAGE:\n\
         \x20   gansi new <name>           Generate a new Yse desktop application\n\
         \x20   gansi new --local <yse-path> <name>\n\
         \x20                              Generate against a local Yse checkout (facade)\n\
         \x20   gansi setup                Install Qt 6 automatically\n\
         \x20   gansi dev                  Build and run the current project\n\
         \x20   gansi test                 Run the current project's tests\n\
         \x20   gansi bundle               Build a release and produce a platform bundle\n\
         \x20   gansi version              Print the version\n"
    );
}

fn command_setup(_args: &[String]) -> Result<(), String> {
    let base =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let prefix = qt::ensure_qt(&base)?;
    println!("Qt 6 ready at {}", prefix.display());
    println!(
        "Run `gansi dev` to build and run your application, or `gansi new <name>` to scaffold one."
    );
    Ok(())
}

fn command_new(args: &[String]) -> Result<(), String> {
    let (name, local_path) = if args.first().map(String::as_str) == Some("--local") {
        if args.len() < 3 {
            return Err("`new --local <yse-path> <name>` requires a path and a name".into());
        }
        (args[2].clone(), Some(args[1].clone()))
    } else {
        let Some(name) = args.first() else {
            return Err("`new` requires a project name, e.g. `gansi new hello`".into());
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
         \x20   gansi setup         # install Qt 6 automatically (first time only)\n\
         \x20   gansi dev           # build and run\n\
         \x20   gansi bundle        # build a release bundle",
        project.name,
        target.display(),
        project.name
    );
    Ok(())
}

fn command_dev(args: &[String]) -> Result<(), String> {
    let base =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let qt_prefix = qt::ensure_qt(&base)?;
    let mut cargo_args = vec!["run".to_string()];
    cargo_args.extend_from_slice(args);
    run_cargo_with_qt(&qt_prefix, &cargo_args)
}

fn command_test(_args: &[String]) -> Result<(), String> {
    let base =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let qt_prefix = qt::ensure_qt(&base)?;
    run_cargo_with_qt(&qt_prefix, &["test".to_string()])
}

fn command_bundle(_args: &[String]) -> Result<(), String> {
    let base =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let qt_prefix = qt::ensure_qt(&base)?;
    let project = project::Project::from_manifest("yse.toml")?;
    run_cargo_with_qt(&qt_prefix, &["build".to_string(), "--release".to_string()])?;
    project::bundle(&project)
}

fn run_cargo_with_qt(qt_prefix: &Path, args: &[String]) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command.args(args);
    // Point CMake (used by cxx-qt-build) and pkg-config at the discovered Qt.
    command.env("CMAKE_PREFIX_PATH", qt_prefix);
    let pkg_config = qt_prefix.join("lib").join("pkgconfig");
    command.env("PKG_CONFIG_PATH", pkg_config);
    let status = command
        .status()
        .map_err(|error| format!("failed to run cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}
