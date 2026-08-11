#![forbid(unsafe_code)]

//! `gansi` — the Yse developer toolchain.
//!
//! Install with `cargo install --path crates/gansi`, then:
//!
//! - `gansi create <name>` — generate a new Yse desktop application.
//! - `gansi run [--release] [-- <args>...]` — build and run the current project.
//! - `gansi test [-- <args>...]` — run the current project's tests.
//! - `gansi build [--debug] [-- <args>...]` — build and bundle the current project.
//! - `gansi analyze [-- <args>...]` — run Clippy with warnings denied.
//! - `gansi format [--check]` — format or verify the project.
//! - `gansi doctor` — check local dependencies and Qt SDK status.

mod project;
mod template;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha1::Digest;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_QT_VERSION: &str = "6.8.3";
const QT_TARGET: &str = "desktop";
#[derive(Clone, Copy)]
struct QtMirror {
    label: &'static str,
    base_url: &'static str,
    root_path: &'static str,
}

const QT_SETUP_MIRRORS: [QtMirror; 3] = [
    QtMirror {
        label: "download.qt.io",
        base_url: "https://download.qt.io",
        root_path: "online/qtsdkrepository",
    },
    QtMirror {
        label: "mirrors.ustc.edu.cn",
        base_url: "https://mirrors.ustc.edu.cn/qtproject",
        root_path: "online/qtsdkrepository",
    },
    QtMirror {
        label: "mirrors.tuna.tsinghua.edu.cn",
        base_url: "https://mirrors.tuna.tsinghua.edu.cn/qt",
        root_path: "online/qtsdkrepository",
    },
];

fn configured_qt_setup_mirrors() -> Vec<QtMirror> {
    let Some(raw) = env::var("GANSI_SETUP_MIRRORS").ok() else {
        return QT_SETUP_MIRRORS.to_vec();
    };
    let mut mirrors = Vec::new();
    for token in raw
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let token_lower = token.to_ascii_lowercase();
        if token_lower.contains("download.qt.io") || token_lower == "download" {
            mirrors.push(QT_SETUP_MIRRORS[0]);
        } else if token_lower.contains("ustc") || token_lower == "ustc" {
            mirrors.push(QT_SETUP_MIRRORS[1]);
        } else if token_lower.contains("tuna") || token_lower == "tuna" {
            mirrors.push(QT_SETUP_MIRRORS[2]);
        } else {
            println!("warning: unknown mirror selector `{token}`, ignored");
        }
    }
    if mirrors.is_empty() {
        return QT_SETUP_MIRRORS.to_vec();
    }
    mirrors
}

#[derive(Clone, Copy)]
struct QtVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ArchiveChecksum {
    Sha256(String),
    Sha1(String),
}

/// Host platform. Replaces the previous stringly-typed `detect_platform`
/// so platform decisions are exhaustive at compile time.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    const fn detect() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        }
    }
}

impl QtVersion {
    fn parse(value: &str) -> Result<Self, String> {
        let mut value = value.trim();
        if let Some((left, _)) = value.split_once('-') {
            value = left;
        }
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() < 2 {
            return Err(format!("unsupported version format `{value}`"));
        }
        let major = parts[0]
            .parse::<u32>()
            .map_err(|error| format!("invalid major version `{value}`: {error}"))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|error| format!("invalid minor version `{value}`: {error}"))?;
        let patch = parts
            .get(2)
            .copied()
            .unwrap_or("0")
            .parse::<u32>()
            .map_err(|error| format!("invalid patch version `{value}`: {error}"))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    fn short(&self) -> String {
        // Qt online repository naming: Qt 5 uses `qt{major}{minor}` (no
        // patch, e.g. 5.15.7 -> "515"); Qt 6 uses `qt6_{major}{minor}{patch}`
        // (e.g. 6.8.3 -> "683", 6.8.0 -> "680").
        if self.major <= 5 {
            format!("{}{}", self.major, self.minor)
        } else {
            format!("{}{}{}", self.major, self.minor, self.patch)
        }
    }
}

#[derive(Parser)]
#[command(
    name = "gansi",
    version = VERSION,
    about = "Project tooling for Yse + native Qt apps."
)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new project against a local Yse checkout.
    #[command(name = "create", alias = "new")]
    Create {
        name: String,
        #[arg(long, help = "Path to a local Yse checkout to pin with `--path`")]
        local: Option<String>,
    },

    /// Build and run the current project.
    #[command(name = "run", alias = "dev")]
    Run {
        #[arg(long)]
        release: bool,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Run tests.
    Test {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Build and bundle the current project.
    #[command(name = "build", alias = "bundle")]
    Build {
        #[arg(long)]
        debug: bool,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Check toolchain, Qt SDK and environment health.
    Doctor {
        #[arg(long)]
        verbose: bool,
    },

    /// Prepare Qt SDK metadata.
    Setup { version: Option<String> },

    /// Remove `target` and `dist`.
    Clean,

    /// Read/write global tool configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Initialize manifest and `.gitignore` only.
    Init,

    /// Print environment summary (`doctor --verbose`).
    Env,

    /// Run Clippy across all project targets with warnings denied.
    Analyze {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Format the project, or verify formatting with `--check`.
    Format {
        #[arg(long)]
        check: bool,
    },

    /// Update dependencies recorded in Cargo.lock.
    Upgrade {
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Add a Cargo dependency to the current project.
    Add {
        package: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Print toolchain version.
    Version,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Print full config.
    List,
    /// Read one value.
    Get { key: String },
    /// Set one value (`qt_roots` accepts comma-separated paths).
    Set { key: String, value: String },
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct GlobalConfig {
    pub(crate) qt_roots: Vec<String>,
    pub(crate) default_qt_version: Option<String>,
    pub(crate) compiler_family: Option<String>,
    pub(crate) target_arch: Option<String>,
}

#[derive(Clone)]
struct HealthCheck {
    label: &'static str,
    found: bool,
    version: Option<String>,
    required: bool,
    suggestion: &'static str,
}

/// Outcome of a `doctor` run, mapped to a process exit code at the boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DoctorStatus {
    AllGood,
    SoftMissing,
    RequiredMissing,
}

impl DoctorStatus {
    const fn exit_code(self) -> i32 {
        match self {
            Self::AllGood => 0,
            Self::SoftMissing => 1,
            Self::RequiredMissing => 2,
        }
    }
}

fn main() {
    let code = match execute(Cli::parse()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            1
        }
    };
    std::process::exit(code);
}

fn execute(cli: Cli) -> Result<i32, String> {
    match cli.command {
        Command::Create { name, local } => {
            command_create(&name, local.as_deref())?;
            Ok(0)
        }
        Command::Run { release, args } => {
            ensure_project_environment()?;
            command_run(release, args)?;
            Ok(0)
        }
        Command::Test { args } => {
            ensure_project_environment()?;
            command_test(args)?;
            Ok(0)
        }
        Command::Build { debug, args } => {
            ensure_project_environment()?;
            command_build(debug, args)?;
            Ok(0)
        }
        Command::Doctor { verbose } => Ok(command_doctor(verbose)?.exit_code()),
        Command::Setup { version } => command_setup(version),
        Command::Clean => {
            command_clean()?;
            Ok(0)
        }
        Command::Config { command } => command_config(command),
        Command::Init => {
            command_init()?;
            Ok(0)
        }
        Command::Env => {
            command_doctor(true)?;
            Ok(0)
        }
        Command::Analyze { args } => {
            ensure_project_environment()?;
            command_analyze(args)?;
            Ok(0)
        }
        Command::Format { check } => {
            command_format(check)?;
            Ok(0)
        }
        Command::Upgrade { args } => {
            command_upgrade(args)?;
            Ok(0)
        }
        Command::Add { package, args } => {
            command_add(&package, args)?;
            Ok(0)
        }
        Command::Version => {
            println!("gansi {VERSION}");
            Ok(0)
        }
    }
}

fn command_create(name: &str, local: Option<&str>) -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let checkout = match local {
        Some(path) => PathBuf::from(path),
        None => discover_yse_checkout(&cwd).ok_or(
            "Yse is not published yet. Run this command inside a Yse checkout or pass \
             `--local <yse-checkout>`.",
        )?,
    };
    let mut project = project::Project::parse_local(name, &checkout.to_string_lossy())?;
    pin_detected_toolchain(&mut project);

    let target = cwd.join(&project.name);
    if target.exists() {
        return Err(format!("directory `{}` already exists", target.display()));
    }
    let staging = cwd.join(format!(".{}.gansi-{}", project.name, std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("cannot clear {}: {error}", staging.display()))?;
    }
    let result = project::write_project(&project, &staging)
        .and_then(|()| add_yse_dependency(&project, &staging))
        .and_then(|()| {
            fs::rename(&staging, &target).map_err(|error| {
                format!(
                    "cannot finalize project {} as {}: {error}",
                    staging.display(),
                    target.display()
                )
            })
        });
    if result.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;
    println!("Generated {} at {}\n", project.name, target.display());
    println!("Next steps:");
    for step in [
        format!("cd {}", project.name),
        "gansi run   # build and run (requires Qt 6 development files)".into(),
        "gansi test  # run tests".into(),
        "gansi build # build a release bundle".into(),
    ] {
        println!("  {step}");
    }
    Ok(())
}

fn add_yse_dependency(project: &project::Project, target: &Path) -> Result<(), String> {
    let yse_path = project
        .local_yse
        .as_ref()
        .ok_or("generated projects require a local Yse checkout until publication")?;
    let args = vec![
        "add".to_string(),
        "yse".to_string(),
        "--path".to_string(),
        yse_path
            .join("crates")
            .join("yse")
            .to_string_lossy()
            .into_owned(),
    ];
    let status = ProcessCommand::new("cargo")
        .args(&args)
        .current_dir(target)
        .status()
        .map_err(|error| format!("cannot run `cargo add yse`: {error}"))?;
    if !status.success() {
        return Err("`cargo add yse --path <checkout>/crates/yse` failed".to_string());
    }
    println!("Added the `yse` dependency.");
    Ok(())
}

fn discover_yse_checkout(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join("crates/yse/Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

fn command_run(release: bool, args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec!["run".to_string()];
    if release {
        cargo_args.push("--release".to_string());
    }
    cargo_args.extend(args);
    run_cargo(&cargo_args)?;
    Ok(())
}

fn command_test(args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec!["test".to_string()];
    cargo_args.extend(args);
    run_cargo(&cargo_args)?;
    Ok(())
}

fn command_analyze(args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec![
        "clippy".to_string(),
        "--all-targets".to_string(),
        "--".to_string(),
        "-D".to_string(),
        "warnings".to_string(),
    ];
    cargo_args.extend(args);
    run_cargo(&cargo_args)
}

fn command_format(check: bool) -> Result<(), String> {
    let mut cargo_args = vec!["fmt".to_string()];
    if check {
        cargo_args.push("--".to_string());
        cargo_args.push("--check".to_string());
    }
    run_cargo(&cargo_args)
}

fn command_upgrade(args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec!["update".to_string()];
    cargo_args.extend(args);
    run_cargo(&cargo_args)
}

fn command_add(package: &str, args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec!["add".to_string(), package.to_string()];
    cargo_args.extend(args);
    run_cargo(&cargo_args)
}

fn command_build(debug: bool, args: Vec<String>) -> Result<(), String> {
    let mut cargo_args = vec!["build".to_string()];
    if !debug {
        cargo_args.push("--release".to_string());
    }
    cargo_args.extend(args);
    run_cargo(&cargo_args)?;

    let project = project::Project::from_manifest(project::MANIFEST_FILE_NAME)?;
    let profile = if debug {
        project::BundleProfile::Debug
    } else {
        project::BundleProfile::Release
    };
    project::bundle(&project, profile)
}

fn command_doctor(verbose: bool) -> Result<DoctorStatus, String> {
    let checks = collect_health_checks();
    render_checks(&checks);
    if verbose {
        render_verbose_environment()?;
    }

    if checks.iter().any(|check| check.required && !check.found) {
        Ok(DoctorStatus::RequiredMissing)
    } else if checks.iter().any(|check| !check.found) {
        Ok(DoctorStatus::SoftMissing)
    } else {
        Ok(DoctorStatus::AllGood)
    }
}

fn command_setup(version: Option<String>) -> Result<i32, String> {
    let mut config = load_global_config()?;
    let manifest = read_manifest().ok();
    let qt_version = version
        .or_else(|| manifest.clone().and_then(|value| value.qt_version))
        .or_else(|| config.default_qt_version.clone())
        .unwrap_or_else(|| DEFAULT_QT_VERSION.to_string());
    let arch = manifest
        .as_ref()
        .and_then(|value| value.target_arch.clone())
        .or_else(|| config.target_arch.clone())
        .unwrap_or_else(default_arch);
    let platform = Platform::detect();
    let expected_root = gansi_home()
        .join("qt")
        .join(&qt_version)
        .join(platform.as_str())
        .join(&arch);

    if has_qmake(&expected_root) {
        register_qt_root(&mut config, &qt_version, &expected_root)?;
        println!("Detected existing Qt layout at {}", expected_root.display());
        return Ok(0);
    }

    if let Some(root) = config.qt_roots.iter().find_map(|root| has_qmake_path(root)) {
        println!("Using existing Qt root: {}", root.display());
        register_qt_root(&mut config, &qt_version, &root)?;
        return Ok(0);
    }

    if let Some(qmake) = locate_qmake() {
        let root = qmake
            .parent()
            .and_then(|bin| bin.parent())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        println!("Found Qt via PATH: {}", root.display());
        register_qt_root(&mut config, &qt_version, &root)?;
        return Ok(0);
    }

    if let Some(root) = install_qt_from_mirrors(&qt_version, platform, &arch, &expected_root)? {
        register_qt_root(&mut config, &qt_version, &root)?;
        println!("Installed and registered Qt: {}", root.display());
        return Ok(0);
    }

    config.default_qt_version = Some(qt_version);
    save_global_config(&config)?;
    println!("No Qt installation discovered for this machine.");
    println!("Expected layout: {}", expected_root.display());
    println!("Set `qt_roots` to an existing Qt root and run `gansi setup` again.");
    println!(
        "Attempted to download Qt from `download.qt.io`, `mirrors.ustc.edu.cn`, and `mirrors.tuna.tsinghua.edu.cn`."
    );
    Ok(1)
}

fn has_qmake_path(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    has_qmake(path).then(|| path.to_path_buf())
}

/// Record a Qt root as the default version and persist the config. Pushes the
/// root only when not already listed, so the call is idempotent.
fn register_qt_root(
    config: &mut GlobalConfig,
    qt_version: &str,
    root: &Path,
) -> Result<(), String> {
    config.default_qt_version = Some(qt_version.to_string());
    if !config.qt_roots.iter().any(|value| Path::new(value) == root) {
        config.qt_roots.push(root.to_string_lossy().into_owned());
    }
    save_global_config(config)
}

#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
fn install_qt_from_mirrors(
    qt_version: &str,
    platform: Platform,
    requested_arch: &str,
    expected_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let requested_version = qt_version;
    if expected_root.exists() && !has_qmake(expected_root) {
        fs::remove_dir_all(expected_root).map_err(|error| {
            format!(
                "cannot clean stale Qt root {}: {error}",
                expected_root.display()
            )
        })?;
    }

    let qt_version = QtVersion::parse(requested_version)?;
    if qt_version.major == 6 && qt_version.minor >= 11 {
        return install_qt_611_from_mirrors(&qt_version, platform, requested_arch, expected_root);
    }
    let version_short = qt_version.short();
    let repo_os_arch = qt_os_arch(platform);
    let qt_arch = qt_archive_arch(platform, requested_arch, &qt_version);
    let mut install_root: Option<PathBuf> = None;
    let qt_full_version = format!(
        "{}.{}.{}",
        qt_version.major, qt_version.minor, qt_version.patch
    );

    let base_folder = format!("qt{}_{}", qt_version.major, version_short);
    // Qt's online repository nested the version directory from 6.8 through
    // 6.10 (`qt6_683/qt6_683/Updates.xml`) and switched to package directory
    // listings starting with 6.11; that layout is handled separately above.
    let use_split = qt_version.major == 6 && (8..=10).contains(&qt_version.minor);
    let version_path = if use_split {
        format!("{base_folder}/{base_folder}")
    } else {
        base_folder
    };
    let updates_path = "Updates.xml".to_string();

    let work_dir = env::temp_dir().join(format!(
        ".gansi_qt_download_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("time error: {error}"))?
            .as_nanos()
    ));
    fs::create_dir_all(&work_dir).map_err(|error| {
        format!(
            "cannot create temporary folder {}: {error}",
            work_dir.display()
        )
    })?;

    let mut last_error: Option<String> = None;
    for mirror in configured_qt_setup_mirrors() {
        let mirror_base = url_join(
            mirror.base_url,
            &[mirror.root_path, repo_os_arch, QT_TARGET, &version_path],
        );
        let updates_url = url_join(&mirror_base, &[&updates_path]);
        let updates_xml = match http_get_text(&updates_url) {
            Ok(xml) => xml,
            Err(error) => {
                println!("  mirror unavailable ({}): {error}", mirror.label);
                last_error = Some(error);
                continue;
            }
        };
        println!("Selected mirror: {} ({})", mirror.label, mirror.base_url);

        let mut candidates = find_matching_qt_candidates(
            &updates_xml,
            &qt_version,
            requested_version,
            &qt_arch,
            true,
        );
        if candidates.is_empty() {
            println!(
                "  no exact qtbase archive found in {}, trying all matching archives",
                mirror.label
            );
            candidates = find_matching_qt_candidates(
                &updates_xml,
                &qt_version,
                requested_version,
                &qt_arch,
                false,
            );
        }
        if candidates.is_empty() {
            println!(
                "  no matching Qt package found in {} Updates.xml",
                mirror.label
            );
            continue;
        }

        candidates.sort_by_key(|candidate| archive_preference(&candidate.archive_name));
        let mut module_failures = 0usize;
        for candidate in candidates {
            let installed = install_qt_candidate(
                &candidate,
                &mirror_base,
                &work_dir,
                expected_root,
                &qt_full_version,
                &qt_arch,
                &mut install_root,
            )?;
            if !installed {
                module_failures += 1;
            }
        }
        if module_failures > 0 {
            println!("  {module_failures} Qt module archive(s) failed; the install is incomplete.");
            let _ = fs::remove_dir_all(expected_root);
            return Err(format!(
                "{module_failures} Qt module archive(s) failed to download/verify/extract; \
                 run `gansi setup` again to retry"
            ));
        }

        if install_root.is_some() {
            break;
        }
    }

    finalize_qt_install(install_root, &work_dir, last_error)
}

/// Post-install bookkeeping shared by every install path: patch library
/// soname symlinks, drop the temporary download folder, and report failure.
fn finalize_qt_install(
    install_root: Option<PathBuf>,
    work_dir: &Path,
    last_error: Option<String>,
) -> Result<Option<PathBuf>, String> {
    if let Some(root) = install_root.as_deref() {
        ensure_qt_library_soname_links(root)?;
        relocate_top_level_libraries(root)?;
    }

    if let Err(error) = fs::remove_dir_all(work_dir) {
        println!(
            "warning: cannot remove temp dir {}: {error}",
            work_dir.display()
        );
    }
    if install_root.is_none() {
        if let Some(error) = last_error {
            println!("Failed all mirrors with latest error: {error}");
        } else {
            println!("No matching archive candidates found in mirror indexes.");
        }
    }
    Ok(install_root)
}

/// Some Qt 6.8-era module archives merge their payload at the Qt root rather
/// than under `lib/` (notably ICU), leaving `libicu*.so` next to `bin/`.
/// Move any top-level shared library into `lib/` so the loader finds it.
fn relocate_top_level_libraries(root: &Path) -> Result<(), String> {
    let lib_dir = root.join("lib");
    fs::create_dir_all(&lib_dir)
        .map_err(|error| format!("cannot create {}: {error}", lib_dir.display()))?;
    for entry in
        fs::read_dir(root).map_err(|error| format!("cannot read {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("lib") || !name.contains(".so") {
            continue;
        }
        let source = entry.path();
        let destination = lib_dir.join(name);
        if source
            .symlink_metadata()
            .map_err(|error| format!("cannot read metadata of {}: {error}", source.display()))?
            .is_dir()
        {
            continue;
        }
        if destination.exists() {
            continue;
        }
        fs::rename(&source, &destination).map_err(|error| {
            format!(
                "cannot move {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

/// Qt 6.11+ dropped the aggregated `Updates.xml`: each package now lives in
/// its own directory that lists `.7z` archives (plus `.sha1` sidecars)
/// directly. Module directories are `qt.qt6.<short>.<arch>`; Windows nests
/// them under an arch folder (`qt6_<short>_<arch>`), the other platforms
/// under a doubled version folder (`qt6_<short>/qt6_<short>`).
fn install_qt_611_from_mirrors(
    qt_version: &QtVersion,
    platform: Platform,
    requested_arch: &str,
    expected_root: &Path,
) -> Result<Option<PathBuf>, String> {
    if expected_root.exists() && !has_qmake(expected_root) {
        fs::remove_dir_all(expected_root).map_err(|error| {
            format!(
                "cannot clean stale Qt root {}: {error}",
                expected_root.display()
            )
        })?;
    }

    let version_short = qt_version.short();
    let qt_arch = qt_archive_arch(platform, requested_arch, qt_version);
    let package_dir = format!("qt.qt6.{version_short}.{qt_arch}");
    let version_folder = match platform {
        Platform::Windows => format!(
            "qt6_{version_short}_{}",
            qt_arch.trim_start_matches("win64_")
        ),
        Platform::Linux | Platform::Macos => format!("qt6_{version_short}/qt6_{version_short}"),
    };
    let qt_full_version = format!(
        "{}.{}.{}",
        qt_version.major, qt_version.minor, qt_version.patch
    );

    let work_dir = env::temp_dir().join(format!(
        ".gansi_qt_download_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("time error: {error}"))?
            .as_nanos()
    ));
    fs::create_dir_all(&work_dir).map_err(|error| {
        format!(
            "cannot create temporary folder {}: {error}",
            work_dir.display()
        )
    })?;

    let mut install_root: Option<PathBuf> = None;
    let mut last_error: Option<String> = None;
    for mirror in configured_qt_setup_mirrors() {
        let mirror_base = url_join(
            mirror.base_url,
            &[
                mirror.root_path,
                qt_os_arch(platform),
                QT_TARGET,
                &version_folder,
            ],
        );
        let package_url = url_join(&mirror_base, &[&package_dir]);
        let html = match http_get_text(&package_url) {
            Ok(html) => html,
            Err(error) => {
                println!("  mirror unavailable ({}): {error}", mirror.label);
                last_error = Some(error);
                continue;
            }
        };
        let archives = list_qt_archives(&html);
        if archives.is_empty() {
            println!(
                "  no Qt archives found at {}, trying next mirror",
                package_url
            );
            last_error = Some("no archives in package directory".to_string());
            continue;
        }
        println!("Selected mirror: {} ({})", mirror.label, mirror.base_url);

        let mut module_failures = 0usize;
        for archive in &archives {
            let archive_url = url_join(&package_url, &[archive]);
            let installed = stage_and_fold_archive(
                archive,
                &archive_url,
                &work_dir,
                expected_root,
                &qt_full_version,
                &qt_arch,
                &mut install_root,
            )?;
            if !installed {
                module_failures += 1;
            }
        }
        if module_failures > 0 {
            // A partial Qt install must not be registered as success: the
            // missing module leaves shared libraries truncated or absent.
            println!("  {module_failures} Qt module archive(s) failed; the install is incomplete.");
            let _ = fs::remove_dir_all(expected_root);
            return Err(format!(
                "{module_failures} Qt module archive(s) failed to download/verify/extract; \
                 run `gansi setup` again to retry"
            ));
        }

        if install_root.is_some() {
            break;
        }
    }

    finalize_qt_install(install_root, &work_dir, last_error)
}

/// List the Qt archives worth installing from a 6.11-style directory listing:
/// `.7z` files that are not mirror-list sidecars and not the `meta` manifest
/// or the `qtdoc` documentation bundle. One archive per module is kept (the
/// first target the listing exposes, e.g. the RHEL build over the older one).
fn list_qt_archives(html: &str) -> Vec<String> {
    let mut archives = Vec::new();
    let mut seen_modules = HashSet::new();
    for href in href_values(html) {
        if !href.ends_with(".7z") || href.contains(".mirrorlist") {
            continue;
        }
        let module = qt_archive_module(&href);
        if matches!(module, "meta" | "qtdoc") || !seen_modules.insert(module.to_string()) {
            continue;
        }
        archives.push(href);
    }
    archives
}

/// Module name of a Qt 6.11+ archive, e.g.
/// `6.11.0-0-202603180534qtbase-…-X86_64.7z` -> `qtbase`, `meta.7z` -> `meta`.
fn qt_archive_module(archive_name: &str) -> &str {
    let name = archive_name.strip_suffix(".7z").unwrap_or(archive_name);
    let Some((_, stamp)) = name.split_once("-0-") else {
        return name;
    };
    let module = stamp.trim_start_matches(char::is_numeric);
    module.split_once('-').map_or(module, |(module, _)| module)
}

/// Values of every `href="…"` attribute in an HTML fragment.
fn href_values(html: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("href=\"") {
        rest = &rest[start + "href=\"".len()..];
        let Some(end) = rest.find('"') else {
            break;
        };
        values.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    values
}

fn parse_checksum_text(text: &str, algorithm: &str) -> Option<ArchiveChecksum> {
    let digest = text.split_whitespace().next()?.trim().to_ascii_lowercase();
    if !digest.chars().all(|value| value.is_ascii_hexdigit()) {
        return None;
    }
    match algorithm {
        "sha256" if digest.len() == 64 => Some(ArchiveChecksum::Sha256(digest)),
        "sha1" if digest.len() == 40 => Some(ArchiveChecksum::Sha1(digest)),
        _ => None,
    }
}

fn fetch_archive_checksum(archive_url: &str) -> Result<ArchiveChecksum, String> {
    let mut failures = Vec::new();
    for algorithm in ["sha256", "sha1"] {
        let checksum_url = format!("{archive_url}.{algorithm}");
        match http_get_text(&checksum_url) {
            Ok(text) => match parse_checksum_text(&text, algorithm) {
                Some(checksum) => return Ok(checksum),
                None => failures.push(format!("invalid {algorithm} sidecar")),
            },
            Err(error) => failures.push(error),
        }
    }
    Err(format!(
        "no valid checksum sidecar for {archive_url}: {}",
        failures.join("; ")
    ))
}
/// Download, verify, extract, and fold one Qt archive into the install root.
/// Failures (download, checksum, extraction) only stop this candidate — the
/// caller moves on to the next one.
#[allow(clippy::too_many_arguments)]
fn stage_and_fold_archive(
    archive_name: &str,
    archive_url: &str,
    work_dir: &Path,
    expected_root: &Path,
    qt_full_version: &str,
    qt_arch: &str,
    install_root: &mut Option<PathBuf>,
) -> Result<bool, String> {
    let checksum = match fetch_archive_checksum(archive_url) {
        Ok(checksum) => checksum,
        Err(error) => {
            println!("  {archive_name} checksum unavailable: {error}");
            return Ok(false);
        }
    };
    let stage = work_dir.join(format!(
        "archive_{}",
        archive_name.replace(['/', '\\'], "_")
    ));
    let mut attempts = 0;
    loop {
        attempts += 1;
        match http_download_file(archive_url, &stage) {
            Ok(()) => break,
            Err(error) if attempts < 3 => {
                // Large Qt archives frequently hit transient disconnects;
                // retry a couple of times before declaring the module failed.
                println!("  {archive_name} download attempt {attempts} failed: {error}; retrying");
                let _ = fs::remove_file(&stage);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            Err(error) => {
                println!("  {archive_name} download failed after {attempts} attempts: {error}");
                return Ok(false);
            }
        }
    }

    match verify_archive_checksum(&stage, &checksum) {
        Ok(true) => {}
        Ok(false) => {
            println!("  {archive_name} checksum mismatch");
            let _ = fs::remove_file(&stage);
            return Ok(false);
        }
        Err(error) => {
            println!("  {archive_name} checksum check failed: {error}");
            let _ = fs::remove_file(&stage);
            return Ok(false);
        }
    }

    let extract_dir = work_dir.join(format!(
        "extract_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("time error: {error}"))?
            .as_nanos()
    ));
    if let Err(error) = extract_archive(&stage, &extract_dir) {
        println!("  failed to extract {archive_name}: {error}");
        let _ = fs::remove_file(&stage);
        return Ok(false);
    }

    if let Some(found_root) = find_qmake_root(&extract_dir).filter(|root| has_qmake(root)) {
        merge_qt_root(&found_root, expected_root)?;
        *install_root = Some(expected_root.to_path_buf());
    } else if let Some(payload_root) = find_qt_payload_root(&extract_dir, qt_full_version, qt_arch)
        && let Some(root) = install_root.as_deref().filter(|root| root.exists())
    {
        if is_top_level_library_payload(&payload_root) {
            copy_dir_all(&payload_root, &root.join("lib"))?;
        } else {
            copy_dir_all(&payload_root, root)?;
        }
    }
    Ok(true)
}

/// Download, verify, and stage one archive candidate, then fold its Qt root
/// into `install_root`. Failures (download, checksum, extraction) only stop
/// this candidate — the caller moves on to the next one.
fn install_qt_candidate(
    candidate: &QtArchiveCandidate,
    mirror_base: &str,
    work_dir: &Path,
    expected_root: &Path,
    qt_full_version: &str,
    qt_arch: &str,
    install_root: &mut Option<PathBuf>,
) -> Result<bool, String> {
    let mut archive_path = String::new();
    if !candidate.package_name.is_empty() {
        archive_path.push_str(&candidate.package_name);
        archive_path.push('/');
    }
    if let Some(location) = candidate.location.as_deref() {
        let location = location.trim_matches('/');
        if !location.is_empty() {
            archive_path.push_str(location);
            archive_path.push('/');
        }
    }
    archive_path.push_str(&candidate.package_version);
    archive_path.push_str(&candidate.archive_name);
    let archive_url = url_join(mirror_base, &[&archive_path]);

    stage_and_fold_archive(
        &candidate.archive_name,
        &archive_url,
        work_dir,
        expected_root,
        qt_full_version,
        qt_arch,
        install_root,
    )
}

/// Move an extracted Qt root into `expected_root`, clearing it first unless
/// it already holds a matching install.
fn merge_qt_root(found_root: &Path, expected_root: &Path) -> Result<(), String> {
    if found_root == expected_root {
        return Ok(());
    }
    if expected_root.exists() {
        fs::remove_dir_all(expected_root)
            .map_err(|error| format!("cannot clear {}: {error}", expected_root.display()))?;
    }
    copy_dir_all(found_root, expected_root)
}

#[derive(Clone)]
struct QtArchiveCandidate {
    archive_name: String,
    package_version: String,
    package_name: String,
    location: Option<String>,
}

/// Trimmed text of the first child element whose tag matches one of `tags`,
/// or `None` when the element is absent or holds only whitespace.
fn child_xml_text<'a, 'input>(node: roxmltree::Node<'a, 'input>, tags: &[&str]) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && tags.contains(&child.tag_name().name()))
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn find_matching_qt_candidates(
    updates_xml: &str,
    qt_version: &QtVersion,
    requested_version: &str,
    qt_arch: &str,
    strict: bool,
) -> Vec<QtArchiveCandidate> {
    let document = match roxmltree::Document::parse(updates_xml) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("warning: cannot parse Updates.xml: {error}");
            return Vec::new();
        }
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for package in document
        .descendants()
        .filter(|node| node.has_tag_name("PackageUpdate"))
    {
        let Some(name) = child_xml_text(package, &["Name"]) else {
            continue;
        };
        let version = child_xml_text(package, &["Version"]).unwrap_or_default();
        if !qt_package_matches(
            &name,
            Some(&version),
            qt_version,
            requested_version,
            qt_arch,
            strict,
        ) {
            continue;
        }
        let Some(downloads) = child_xml_text(package, &["DownloadableArchives"]) else {
            continue;
        };
        let location = child_xml_text(package, &["DownloadLocation"]);
        let is_base_package = is_qt_base_package(&name, qt_version);
        for archive in split_archives(&downloads) {
            if strict {
                if !is_base_package || is_debug_archive(&archive) {
                    continue;
                }
            } else if !is_base_package && !is_qtbase_archive(&archive) {
                continue;
            }
            let key = location.as_deref().map_or_else(
                || format!("{name}|{version}|{archive}"),
                |location| format!("{name}|{location}|{version}|{archive}"),
            );
            if seen.insert(key) {
                candidates.push(QtArchiveCandidate {
                    archive_name: archive,
                    package_version: version.clone(),
                    package_name: name.to_string(),
                    location: location.clone(),
                });
            }
        }
    }
    candidates
}

fn qt_package_matches(
    name: &str,
    package_version: Option<&str>,
    qt_version: &QtVersion,
    requested_version: &str,
    arch: &str,
    strict: bool,
) -> bool {
    if strict && !is_qt_base_package(name, qt_version) {
        return false;
    }
    let name = name.to_lowercase();
    if !name.contains(&arch.to_lowercase()) && !name.ends_with(arch) {
        return false;
    }
    if package_version.is_some_and(|value| qt_version_matches(value, qt_version)) {
        return true;
    }
    version_token_matches(&name, requested_version, qt_version)
}

fn qt_version_matches(value: &str, qt_version: &QtVersion) -> bool {
    let value = value.split_once('-').map_or(value, |(left, _)| left);
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let Ok(major) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(minor) = parts[1].parse::<u32>() else {
        return false;
    };
    if major != qt_version.major || minor != qt_version.minor {
        return false;
    }
    if qt_version.patch == 0 {
        return true;
    }
    let Some(patch) = parts.get(2).and_then(|value| value.parse::<u32>().ok()) else {
        return true;
    };
    patch == qt_version.patch
}

fn version_token_matches(name: &str, requested_version: &str, qt_version: &QtVersion) -> bool {
    let requested = requested_version.to_lowercase();
    let requested_with_underscore = requested.replace('.', "_");
    [
        requested,
        requested_with_underscore,
        qt_version.short(),
        format!("{}{}", qt_version.major, qt_version.minor),
        format!("{}_{}", qt_version.major, qt_version.minor),
        format!("{}.{}", qt_version.major, qt_version.minor),
        format!(
            "{}.{}.{}",
            qt_version.major, qt_version.minor, qt_version.patch
        ),
        format!(
            "{}_{}_{}",
            qt_version.major, qt_version.minor, qt_version.patch
        ),
    ]
    .iter()
    .any(|candidate| name.contains(candidate))
}

fn is_qtbase_archive(name: &str) -> bool {
    name.to_lowercase().contains("qtbase")
}

fn is_debug_archive(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("debug") || name.contains("symbol")
}

fn is_qt_base_package(name: &str, qt_version: &QtVersion) -> bool {
    let version_short = qt_version.short();
    let parts: Vec<&str> = name.split('.').collect();

    if qt_version.major >= 6 {
        if parts.len() != 4 {
            return false;
        }
        return parts[0].eq_ignore_ascii_case("qt")
            && parts[1].eq_ignore_ascii_case(&format!("qt{}", qt_version.major))
            && parts[2].eq_ignore_ascii_case(&version_short);
    }

    parts.len() == 3
        && parts[0].eq_ignore_ascii_case("qt")
        && parts[1].eq_ignore_ascii_case(&version_short)
}

fn split_archives(value: &str) -> Vec<String> {
    value
        .split([',', ';', ' ', '\n'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn archive_preference(name: &str) -> u8 {
    let name = name.to_lowercase();
    if name.ends_with(".zip") {
        0
    } else if name.ends_with(".tar.xz") || name.ends_with(".tar.bz2") || name.ends_with(".tar.gz") {
        2
    } else if name.ends_with(".tar") {
        3
    } else {
        4
    }
}

fn http_get_text(url: &str) -> Result<String, String> {
    let response =
        reqwest::blocking::get(url).map_err(|error| format!("failed to fetch {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch failed: {url} -> HTTP {}", response.status()));
    }
    response
        .text()
        .map_err(|error| format!("failed to read response body {url}: {error}"))
}

fn http_download_file(url: &str, target: &Path) -> Result<(), String> {
    let mut response = reqwest::blocking::get(url)
        .map_err(|error| format!("failed to download {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download failed: {url} -> HTTP {}",
            response.status()
        ));
    }
    let mut output = fs::File::create(target)
        .map_err(|error| format!("cannot create download file {}: {error}", target.display()))?;
    io::copy(&mut response, &mut output)
        .map_err(|error| format!("cannot write download file {}: {error}", target.display()))?;
    Ok(())
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn extract_archive(archive: &Path, target: &Path) -> Result<(), String> {
    fn extract_tar<R: Read>(
        archive: &Path,
        target: &Path,
        decode: impl FnOnce(io::BufReader<fs::File>) -> R,
    ) -> Result<(), String> {
        let file = fs::File::open(archive)
            .map_err(|error| format!("cannot open {}: {error}", archive.display()))?;
        tar::Archive::new(decode(io::BufReader::new(file)))
            .unpack(target)
            .map_err(|error| format!("cannot extract {}: {error}", archive.display()))
    }

    let archive_name = archive
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let archive_name = archive_name.to_lowercase();
    fs::create_dir_all(target)
        .map_err(|error| format!("cannot create extraction dir {}: {error}", target.display()))?;

    if archive_name.ends_with(".zip") {
        let mut file = fs::File::open(archive)
            .map_err(|error| format!("cannot open {}: {error}", archive.display()))?;
        let mut zip = zip::ZipArchive::new(&mut file)
            .map_err(|error| format!("cannot read zip {}: {error}", archive.display()))?;
        zip.extract(target)
            .map_err(|error| format!("cannot extract zip {}: {error}", archive.display()))
    } else if archive_name.ends_with(".tar.xz") {
        extract_tar(archive, target, xz2::read::XzDecoder::new)
    } else if archive_name.ends_with(".tar.gz") {
        extract_tar(archive, target, flate2::read::GzDecoder::new)
    } else if archive_name.ends_with(".tar.bz2") {
        extract_tar(archive, target, bzip2::read::BzDecoder::new)
    } else if archive_name.ends_with(".tar") {
        extract_tar(archive, target, io::BufReader::new)
    } else if archive_name.ends_with(".7z") {
        extract_7z(archive, target)
    } else {
        Err(format!("unsupported archive format: {archive_name}"))
    }
}

/// Extract a 7z archive with a pure-Rust decoder. Qt's online archives are
/// `.7z`, so this closes the last gap in the `setup` download-and-install
/// flow. Each entry is written relative to `target`; absolute or
/// parent-traversing paths are rejected.
fn extract_7z(archive: &Path, target: &Path) -> Result<(), String> {
    // Pass 1: collect every entry name. The 7z format stores Unix symlinks as
    // regular entries whose content is the link target, so we need the full
    // name set to resolve them after extraction.
    let names = {
        let mut reader =
            sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
                .map_err(|error| format!("cannot read 7z {}: {error}", archive.display()))?;
        let mut names = HashSet::new();
        reader
            .for_each_entries(|entry, _input| {
                names.insert(entry.name.clone());
                Ok(true)
            })
            .map_err(|error| format!("cannot list 7z {}: {error}", archive.display()))?;
        names
    };

    let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty())
        .map_err(|error| format!("cannot read 7z {}: {error}", archive.display()))?;
    let mut failure: Option<String> = None;
    reader
        .for_each_entries(|entry, input| {
            if failure.is_some() {
                return Ok(false);
            }
            let relative = Path::new(&entry.name);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                failure = Some(format!("unsafe path in archive: {}", entry.name));
                return Ok(false);
            }
            let destination = target.join(relative);
            if entry.is_directory {
                if let Err(error) = fs::create_dir_all(&destination) {
                    failure = Some(format!("cannot create {}: {error}", destination.display()));
                    return Ok(false);
                }
                return Ok(true);
            }
            if !entry.has_stream {
                return Ok(true);
            }
            // Only short entries can be stored symlinks; read those fully.
            if entry.size <= 512 {
                let mut payload = Vec::new();
                if let Err(error) = input.read_to_end(&mut payload) {
                    failure = Some(format!("cannot read entry {}: {error}", entry.name));
                    return Ok(false);
                }
                if let Some(link_target) = stored_symlink_target(&payload, &entry.name, &names) {
                    if destination.exists() {
                        let _ = fs::remove_file(&destination);
                    }
                    if let Some(parent) = destination.parent()
                        && let Err(error) = fs::create_dir_all(parent)
                    {
                        failure = Some(format!("cannot create {}: {error}", parent.display()));
                        return Ok(false);
                    }
                    if let Err(error) = make_symlink(Path::new(link_target), &destination) {
                        failure = Some(format!(
                            "cannot create symlink {}: {error}",
                            destination.display()
                        ));
                        return Ok(false);
                    }
                    return Ok(true);
                }
                // Not a symlink: write the buffered payload.
                if let Some(parent) = destination.parent()
                    && let Err(error) = fs::create_dir_all(parent)
                {
                    failure = Some(format!("cannot create {}: {error}", parent.display()));
                    return Ok(false);
                }
                let mut output = match fs::File::create(&destination) {
                    Ok(output) => output,
                    Err(error) => {
                        failure = Some(format!("cannot create {}: {error}", destination.display()));
                        return Ok(false);
                    }
                };
                if let Err(error) = io::Write::write_all(&mut output, &payload) {
                    failure = Some(format!("cannot write {}: {error}", destination.display()));
                    return Ok(false);
                }
                drop(output);
                if make_executable(&destination).is_err() {
                    failure = Some(format!(
                        "cannot chmod {} on a Unix host",
                        destination.display()
                    ));
                    return Ok(false);
                }
                return Ok(true);
            }
            if let Some(parent) = destination.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                failure = Some(format!("cannot create {}: {error}", parent.display()));
                return Ok(false);
            }
            let mut output = match fs::File::create(&destination) {
                Ok(output) => output,
                Err(error) => {
                    failure = Some(format!("cannot create {}: {error}", destination.display()));
                    return Ok(false);
                }
            };
            if let Err(error) = io::copy(input, &mut output) {
                failure = Some(format!("cannot write {}: {error}", destination.display()));
                return Ok(false);
            }
            drop(output);
            // Qt 7z archives do not carry Unix permission bits; without this
            // fixup every extracted binary (qmake, tools, plugins) would land
            // as mode 0644 and fail to execute on Linux.
            if make_executable(&destination).is_err() {
                failure = Some(format!(
                    "cannot chmod {} on a Unix host",
                    destination.display()
                ));
                return Ok(false);
            }
            Ok(true)
        })
        .map_err(|error| format!("7z decode failed: {error}"))?;
    failure.map_or(Ok(()), Err)
}

/// Decide whether a 7z entry is a stored symlink. Qt archives encode links as
/// a short payload naming another entry in the same archive (e.g.
/// `libQt6Core.so.6.8.3`); anything larger than a path cannot be one.
fn stored_symlink_target<'a>(
    payload: &'a [u8],
    entry_name: &str,
    names: &HashSet<String>,
) -> Option<&'a str> {
    if payload.is_empty() || payload.len() > 512 {
        return None;
    }
    let text = std::str::from_utf8(payload)
        .ok()?
        .trim_end_matches(['\n', '\r']);
    if text.is_empty() || text.contains('\0') {
        return None;
    }
    let target = Path::new(text);
    if target.is_absolute()
        || target
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    // The target must name a sibling file in the same directory of this
    // archive (Qt soname links are plain relative names).
    let entry_path = Path::new(entry_name);
    let expected = entry_path
        .parent()
        .map(|parent| parent.join(text).to_string_lossy().into_owned())
        .unwrap_or_else(|| text.to_string());
    if names.contains(&expected) {
        Some(text)
    } else {
        None
    }
}

/// On Unix, mark a freshly extracted regular file executable (0755). On
/// Windows the attribute is meaningless and skipped.
#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn verify_archive_checksum(path: &Path, checksum: &ArchiveChecksum) -> Result<bool, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut buffer = [0u8; 16 * 1024];
    let actual = match checksum {
        ArchiveChecksum::Sha256(_) => {
            let mut hasher = sha2::Sha256::new();
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            format!("{:x}", hasher.finalize())
        }
        ArchiveChecksum::Sha1(_) => {
            let mut hasher = sha1::Sha1::new();
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            format!("{:x}", hasher.finalize())
        }
    };
    let expected = match checksum {
        ArchiveChecksum::Sha256(expected) | ArchiveChecksum::Sha1(expected) => expected,
    };
    Ok(actual == *expected)
}

#[cfg(unix)]
fn ensure_qt_library_soname_links(root: &Path) -> Result<(), String> {
    for dirname in ["lib", "lib64"] {
        let dir = root.join(dirname);
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir)
            .map_err(|error| format!("cannot list {dir}: {error}", dir = dir.display()))?
        {
            let entry = entry
                .map_err(|error| format!("cannot read {dir}: {error}", dir = dir.display()))?;
            if !entry
                .file_type()
                .map_err(|error| {
                    format!("cannot read file type {}: {error}", entry.path().display())
                })?
                .is_file()
            {
                continue;
            }
            let Some(filename) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Some((base, version)) = filename.rsplit_once(".so.") else {
                continue;
            };
            if !version
                .split('.')
                .all(|value| value.chars().all(|value| value.is_ascii_digit()))
            {
                continue;
            }
            if !version.contains('.') {
                continue;
            }
            let major = version.split('.').next().unwrap_or_default();
            if major.is_empty() {
                continue;
            }
            let soname = format!("{base}.so.{major}");
            let link = dir.join(&soname);
            if link.exists() {
                continue;
            }
            std::os::unix::fs::symlink(entry.file_name(), &link).map_err(|error| {
                format!(
                    "cannot create symlink {} -> {}: {error}",
                    link.display(),
                    entry.file_name().to_string_lossy()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_qt_library_soname_links(_: &Path) -> Result<(), String> {
    Ok(())
}

fn find_qmake_root(start: &Path) -> Option<PathBuf> {
    let mut stack = vec![start.to_path_buf()];
    while let Some(path) = stack.pop() {
        if has_qmake(&path) {
            return Some(path);
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Some(name) = entry_path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name == "." || name == ".." || name.starts_with('.') {
                continue;
            }
            if let Ok(metadata) = entry_path.symlink_metadata()
                && (metadata.is_file() || metadata.is_symlink())
            {
                continue;
            }
            if entry
                .file_type()
                .ok()
                .is_some_and(|file_type| file_type.is_dir())
            {
                stack.push(entry_path);
            }
        }
    }
    None
}

fn find_qt_payload_root(extract_dir: &Path, qt_version: &str, qt_arch: &str) -> Option<PathBuf> {
    let version_root = extract_dir.join(qt_version);
    if version_root.exists() {
        let qt_root = version_root.join(qt_arch);
        if qt_root.exists() && (qt_root.join("bin").is_dir() || qt_root.join("lib").is_dir()) {
            return Some(qt_root);
        }
        if version_root.join("bin").is_dir() || version_root.join("lib").is_dir() {
            return Some(version_root);
        }
    }
    if has_direct_file_payload(extract_dir) {
        return Some(extract_dir.to_path_buf());
    }

    let mut stack = vec![extract_dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        if !path.exists() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            if let Some(name) = entry_path.file_name().and_then(|value| value.to_str())
                && name.starts_with('.')
            {
                continue;
            }
            if entry_path.join("bin").is_dir() || entry_path.join("lib").is_dir() {
                return Some(entry_path);
            }
            stack.push(entry_path);
        }
    }
    None
}

fn has_direct_file_payload(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            return true;
        }
    }
    false
}

fn is_top_level_library_payload(path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    let mut file_count = 0usize;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            return false;
        }
        let Ok(name) = entry.file_name().into_string() else {
            return false;
        };
        if !name.starts_with("lib") || !name.contains(".so") {
            return false;
        }
        file_count += 1;
    }
    file_count > 0
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        if dst.exists() {
            fs::remove_file(dst).map_err(|error| {
                format!("cannot replace existing file {}: {error}", dst.display())
            })?;
        }
        fs::copy(src, dst).map_err(|error| {
            format!(
                "cannot copy {} to {}: {error}",
                src.display(),
                dst.display()
            )
        })?;
        return Ok(());
    }
    if !dst.exists() {
        fs::create_dir_all(dst)
            .map_err(|error| format!("cannot create {}: {error}", dst.display()))?;
    }
    for entry in fs::read_dir(src)
        .map_err(|error| format!("cannot read source directory {}: {error}", src.display()))?
    {
        let entry = entry
            .map_err(|error| format!("cannot read source entry in {}: {error}", src.display()))?;
        let source = entry.path();
        let destination = dst.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "cannot read metadata for source path {}: {error}",
                source.display()
            )
        })?;
        if metadata.is_dir() {
            if !destination.exists() {
                fs::create_dir_all(&destination)
                    .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
            }
            copy_dir_all(&source, &destination)?;
        } else if metadata.file_type().is_symlink() {
            // fs::copy dereferences symlinks, turning Qt's soname links into
            // plain files containing the target name. Recreate the link.
            let target = fs::read_link(&source)
                .map_err(|error| format!("cannot read symlink {}: {error}", source.display()))?;
            if destination.exists() {
                let _ = fs::remove_file(&destination);
            }
            make_symlink(&target, &destination).map_err(|error| {
                format!(
                    "cannot create symlink {} -> {}: {error}",
                    destination.display(),
                    target.display()
                )
            })?;
        } else {
            if destination.exists() {
                let _ = fs::remove_file(&destination);
            }
            fs::copy(&source, &destination).map_err(|error| {
                format!(
                    "cannot copy {} to {}: {error}",
                    source.display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn make_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    // Windows directory junctions need privileges; a plain copy is a safe
    // fallback because Qt on Windows ships real files, not soname links.
    fs::copy(target, link).map(|_| ())
}

fn url_join(base: &str, segments: &[&str]) -> String {
    let mut out = base.trim_end_matches('/').to_string();
    for segment in segments {
        for part in segment.split('/').filter(|part| !part.is_empty()) {
            out.push('/');
            out.push_str(part.trim_matches('/'));
        }
    }
    out
}

const fn qt_os_arch(platform: Platform) -> &'static str {
    match platform {
        Platform::Windows => "windows_x86",
        Platform::Macos => "mac_x64",
        Platform::Linux => "linux_x64",
    }
}

fn qt_archive_arch(platform: Platform, requested_arch: &str, qt_version: &QtVersion) -> String {
    match platform {
        Platform::Windows => {
            if qt_version.major == 6 && qt_version.minor >= 8 || qt_version.major > 6 {
                "win64_msvc2022_64".to_string()
            } else {
                "win64_msvc2019_64".to_string()
            }
        }
        Platform::Macos => "clang_64".to_string(),
        Platform::Linux => {
            if requested_arch == "aarch64" {
                "gcc_arm64".to_string()
            } else {
                "gcc_64".to_string()
            }
        }
    }
}

fn command_clean() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let target_dir = cargo_target_dir(&cwd, env::var_os("CARGO_TARGET_DIR").as_deref())?;
    let reclaimed = reclaim_targets(&[target_dir, cwd.join("dist")])?;
    println!("Reclaimed approximately {reclaimed} bytes");
    Ok(())
}

fn cargo_target_dir(cwd: &Path, configured: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    let Some(configured) = configured else {
        return Ok(cwd.join("target"));
    };
    if configured.is_empty() {
        return Err("CARGO_TARGET_DIR must not be empty".to_string());
    }
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        Ok(configured)
    } else {
        Ok(cwd.join(configured))
    }
}

fn command_config(command: ConfigCommand) -> Result<i32, String> {
    let mut config = load_global_config()?;
    match command {
        ConfigCommand::List => {
            let text = toml::to_string_pretty(&config)
                .map_err(|error| format!("cannot serialize config: {error}"))?;
            println!("{text}");
            Ok(0)
        }
        ConfigCommand::Get { key } => {
            let value = match key.as_str() {
                "qt_roots" => config.qt_roots.join(","),
                "default_qt_version" => config.default_qt_version.unwrap_or_default(),
                "compiler_family" => config.compiler_family.unwrap_or_default(),
                "target_arch" => config.target_arch.unwrap_or_default(),
                _ => return Err(format!("unknown key `{key}`")),
            };
            println!("{value}");
            Ok(0)
        }
        ConfigCommand::Set { key, value } => {
            match key.as_str() {
                "qt_roots" => {
                    let qt_roots: Vec<String> = value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .collect();
                    let mut uniq_roots = Vec::new();
                    for root in qt_roots {
                        if !uniq_roots.contains(&root) {
                            uniq_roots.push(root);
                        }
                    }
                    config.qt_roots = uniq_roots;
                }
                "default_qt_version" => {
                    config.default_qt_version = to_option(&value);
                }
                "compiler_family" => {
                    config.compiler_family = to_option(&value);
                }
                "target_arch" => {
                    config.target_arch = to_option(&value);
                }
                _ => return Err(format!("unknown key `{key}`")),
            }
            save_global_config(&config)?;
            println!("Updated.");
            Ok(0)
        }
    }
}

fn command_init() -> Result<(), String> {
    let cwd =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let cwd_name = cwd
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("invalid current directory name")?;
    let mut project = project::Project::parse(cwd_name)?;
    pin_detected_toolchain(&mut project);

    let manifest = cwd.join(project::MANIFEST_FILE_NAME);
    if manifest.exists() {
        println!("already exists: {}", manifest.display());
    } else {
        fs::write(&manifest, template::render(template::GANSI_TOML, &project))
            .map_err(|error| format!("cannot write {}: {error}", manifest.display()))?;
        println!("created {}", manifest.display());
    }

    let gitignore = cwd.join(".gitignore");
    if gitignore.exists() {
        println!("already exists: {}", gitignore.display());
    } else {
        fs::write(&gitignore, template::render(template::GITIGNORE, &project))
            .map_err(|error| format!("cannot write {}: {error}", gitignore.display()))?;
        println!("created {}", gitignore.display());
    }
    Ok(())
}

fn ensure_project_environment() -> Result<(), String> {
    match command_doctor(false)? {
        DoctorStatus::RequiredMissing => Err(String::from(
            "required environment checks failed. Run `gansi doctor`.",
        )),
        DoctorStatus::SoftMissing => {
            eprintln!("Warning: environment has soft-missing items. Build may still work.");
            Ok(())
        }
        DoctorStatus::AllGood => Ok(()),
    }?;
    validate_project_toolchain()
}

fn pin_detected_toolchain(project: &mut project::Project) {
    let qt_version = detected_qt_version().unwrap_or_else(|| "6".to_string());
    project.pin_toolchain(qt_version, detected_compiler_family(), env::consts::ARCH);
}

fn validate_project_toolchain() -> Result<(), String> {
    let manifest = read_manifest()?;
    validate_toolchain_values(
        &manifest,
        detected_qt_version().as_deref(),
        &detected_compiler_family(),
        env::consts::ARCH,
    )
}

fn validate_toolchain_values(
    manifest: &project::ManifestProject,
    qt_version: Option<&str>,
    compiler_family: &str,
    target_arch: &str,
) -> Result<(), String> {
    let mut mismatches = Vec::new();

    if let Some(expected) = manifest.qt_version.as_deref()
        && let Some(actual) = qt_version
        && expected != actual
    {
        mismatches.push(format!(
            "Qt {expected} is pinned, but qmake reports Qt {actual}"
        ));
    }
    if let Some(expected) = manifest.compiler_family.as_deref()
        && expected != compiler_family
    {
        mismatches.push(format!(
            "compiler family {expected} is pinned, but {compiler_family} was detected"
        ));
    }
    if let Some(expected) = manifest.target_arch.as_deref()
        && expected != target_arch
    {
        mismatches.push(format!(
            "target architecture {expected} is pinned, but this host is {target_arch}"
        ));
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "project toolchain does not match gansi.toml:\n  - {}\nUpdate the pins intentionally or use the matching SDK/toolchain.",
            mismatches.join("\n  - ")
        ))
    }
}

fn detected_qt_version() -> Option<String> {
    let config = load_global_config().ok()?;
    let qmake = resolve_qmake(&config)?;
    let output = ProcessCommand::new(qmake)
        .args(["-query", "QT_VERSION"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
}

fn detected_compiler_family() -> String {
    if cfg!(target_env = "msvc") {
        return "msvc".to_string();
    }
    let version = tool_version("c++").unwrap_or_default().to_ascii_lowercase();
    if version.contains("clang") {
        "clang".to_string()
    } else {
        "gcc".to_string()
    }
}

fn run_cargo(args: &[String]) -> Result<(), String> {
    let mut command = ProcessCommand::new("cargo");
    command.args(args);
    apply_qt_env(&mut command)?;
    let status = command
        .status()
        .map_err(|error| format!("failed to run cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

fn apply_qt_env(command: &mut ProcessCommand) -> Result<(), String> {
    let config = load_global_config()?;
    if let Some(root) = candidate_qt_roots(&config).into_iter().next() {
        let qmake = root.join("bin").join(qmake_binary_name());
        let path = env::var_os("PATH").unwrap_or_default();
        let mut entries: Vec<PathBuf> = vec![root.join("bin")];
        entries.extend(env::split_paths(&path));
        let path = env::join_paths(entries).map_err(|error| format!("invalid PATH: {error}"))?;
        command.env("PATH", path);
        command.env("QMAKE", qmake);
        command.env("CMAKE_PREFIX_PATH", &root);
        if cfg!(target_os = "linux") {
            let mut lib_dirs = Vec::new();
            if root.join("lib").exists() {
                lib_dirs.push(root.join("lib"));
            }
            if root.join("lib64").exists() {
                lib_dirs.push(root.join("lib64"));
            }
            let mut entries = env::split_paths(&env::var_os("LD_LIBRARY_PATH").unwrap_or_default())
                .collect::<Vec<_>>();
            entries.extend(lib_dirs);
            if !entries.is_empty() {
                let library_path = env::join_paths(entries)
                    .map_err(|error| format!("invalid LD_LIBRARY_PATH: {error}"))?;
                command.env("LD_LIBRARY_PATH", library_path);
            }
        }
    }
    Ok(())
}

/// Qt roots to try, in order: configured `qt_roots`, the `QMAKE` env var, and
/// a `qmake` found on `PATH` (only when the others came up empty).
fn candidate_qt_roots(config: &GlobalConfig) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = config
        .qt_roots
        .iter()
        .map(PathBuf::from)
        .filter(|root| has_qmake(root))
        .collect();
    if let Some(qmake) = env::var_os("QMAKE").map(PathBuf::from)
        && let Some(root) = qt_root_from_qmake(&qmake)
    {
        roots.push(root);
    }
    if roots.is_empty()
        && let Some(root) = locate_qmake().and_then(|qmake| qt_root_from_qmake(&qmake))
    {
        roots.push(root);
    }
    roots
}

fn collect_health_checks() -> Vec<HealthCheck> {
    let mut checks = vec![
        check_tool("rustc", true, "Install Rust via rustup."),
        check_tool("cargo", true, "Install Rust via rustup."),
        check_tool("c++", true, "Install a C++17 compiler (GCC or Clang)."),
        check_tool("cmake", true, "Install CMake."),
        check_tool("ninja", true, "Install Ninja."),
        check_tool("pkg-config", true, "Install pkg-config."),
    ];
    if cfg!(target_os = "linux") {
        checks.extend([
            check_tool("ldd", true, "Install glibc development tools."),
            check_tool("readelf", true, "Install GNU binutils."),
        ]);
    }
    let config = load_global_config().unwrap_or_default();
    if let Some(qmake) = resolve_qmake(&config) {
        checks.push(HealthCheck {
            label: "qmake",
            found: true,
            version: qmake_version(&qmake),
            required: true,
            suggestion: "n/a",
        });
        checks.push(check_qt_platform_plugin(&qmake));
    } else {
        checks.push(HealthCheck {
            label: "qmake",
            found: false,
            version: None,
            required: true,
            suggestion: "Install Qt 6 development files or run `gansi setup`.",
        });
        checks.push(HealthCheck {
            label: "Qt platform plugin",
            found: false,
            version: None,
            required: true,
            suggestion: "Install the Qt 6 platform plugins package.",
        });
    }

    checks
}

fn check_qt_platform_plugin(qmake: &Path) -> HealthCheck {
    let plugins = ProcessCommand::new(qmake)
        .args(["-query", "QT_INSTALL_PLUGINS"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
        .filter(|path| path.join("platforms").is_dir());

    HealthCheck {
        label: "Qt platform plugin",
        found: plugins.is_some(),
        version: plugins.map(|path| path.display().to_string()),
        required: true,
        suggestion: "Install the Qt 6 platform plugins package.",
    }
}

fn check_tool(command: &'static str, required: bool, suggestion: &'static str) -> HealthCheck {
    tool_version(command).map_or_else(
        || HealthCheck {
            label: command,
            found: false,
            version: None,
            required,
            suggestion,
        },
        |version| HealthCheck {
            label: command,
            found: true,
            version: Some(version),
            required,
            suggestion,
        },
    )
}

fn render_checks(checks: &[HealthCheck]) {
    println!("gansi {VERSION} environment");
    for check in checks {
        if check.found {
            if let Some(version) = &check.version {
                println!("  {:16} [ok]   {version}", check.label);
            } else {
                println!("  {:16} [ok]", check.label);
            }
        } else {
            println!("  {:16} [miss] {}", check.label, check.suggestion);
        }
    }
}

fn render_verbose_environment() -> Result<(), String> {
    println!("host:");
    println!("  os: {}", env::consts::OS);
    println!("  arch: {}", env::consts::ARCH);
    if let Ok(os_release) = fs::read_to_string("/etc/os-release")
        && let Some(pretty_name) = os_release
            .lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME="))
    {
        println!("  distribution: {}", pretty_name.trim_matches('"'));
    }
    for key in ["XDG_SESSION_TYPE", "QT_QPA_PLATFORM", "QMAKE"] {
        if let Ok(value) = env::var(key) {
            println!("  {key}: {value}");
        }
    }
    let config = load_global_config()?;
    println!("config: {}", config_path().display());
    if config.qt_roots.is_empty() {
        println!("  qt_roots: <empty>");
    } else {
        for root in config.qt_roots {
            println!("  qt_roots: {root}");
        }
    }
    if let Some(version) = config.default_qt_version {
        println!("  default_qt_version: {version}");
    }
    if let Some(family) = config.compiler_family {
        println!("  compiler_family: {family}");
    }
    if let Some(arch) = config.target_arch {
        println!("  target_arch: {arch}");
    }
    if let Ok(manifest) = read_manifest() {
        println!("project: {}", manifest.name);
        if let Some(version) = manifest.qt_version {
            println!("  qt_version: {version}");
        }
        if let Some(arch) = manifest.target_arch {
            println!("  project.target_arch: {arch}");
        }
        if let Some(family) = manifest.compiler_family {
            println!("  compiler_family: {family}");
        }
    }
    Ok(())
}

fn qmake_version(path: &Path) -> Option<String> {
    let root = qt_root_from_qmake(path)?;
    let version_file = root
        .join("lib")
        .join("cmake")
        .join("Qt6Core")
        .join("Qt6CoreConfigVersion.cmake");
    let text = fs::read_to_string(version_file).ok()?;
    text.lines()
        .find_map(parse_cmake_package_version)
        .or_else(|| Some("installed".to_string()))
}

/// The `<root>/bin/qmake` path pins its Qt root one level up: `<root>/bin`.
fn qt_root_from_qmake(qmake: &Path) -> Option<PathBuf> {
    let root = qmake.parent()?.parent()?.to_path_buf();
    has_qmake(&root).then_some(root)
}

fn parse_cmake_package_version(line: &str) -> Option<String> {
    let value = line
        .trim()
        .strip_prefix("set(PACKAGE_VERSION")?
        .strip_suffix(')')?
        .trim()
        .trim_matches('"');
    (!value.is_empty()).then(|| value.to_string())
}

fn tool_version(command: &str) -> Option<String> {
    let command_path = find_command(command)?;
    let output = ProcessCommand::new(command_path)
        .arg("--version")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    text.lines().find_map(|line| {
        line.split_whitespace().find_map(|value| {
            let has_digit = value.chars().any(|value| value.is_ascii_digit());
            if has_digit {
                Some(value.to_string())
            } else {
                None
            }
        })
    })
}

fn locate_qmake() -> Option<PathBuf> {
    if let Ok(qmake) = env::var("QMAKE") {
        let qmake = PathBuf::from(qmake);
        if qmake.exists() {
            return Some(qmake);
        }
    }
    if let Some(qmake) = find_command("qmake6") {
        return Some(qmake);
    }
    if let Some(qmake) = find_command("qmake") {
        return Some(qmake);
    }
    None
}

fn resolve_qmake(config: &GlobalConfig) -> Option<PathBuf> {
    config
        .qt_roots
        .iter()
        .map(PathBuf::from)
        .find(|root| has_qmake(root))
        .map(|root| root.join("bin").join(qmake_binary_name()))
        .or_else(locate_qmake)
}

fn find_command(name: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(windows) {
        vec![
            name.to_string(),
            format!("{name}.exe"),
            format!("{name}.bat"),
            format!("{name}.cmd"),
        ]
    } else {
        vec![name.to_string()]
    };
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        for candidate in &candidates {
            let candidate = dir.join(candidate);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn has_qmake(root: &Path) -> bool {
    root.join("bin").join(qmake_binary_name()).exists()
}

const fn qmake_binary_name() -> &'static str {
    if cfg!(windows) { "qmake.exe" } else { "qmake" }
}

fn default_arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "aarch64".to_string()
    } else {
        "x86_64".to_string()
    }
}

fn reclaim_targets(paths: &[PathBuf]) -> Result<u64, String> {
    let mut bytes = 0u64;
    for path in paths {
        if path.exists() {
            bytes += directory_size(path)?;
            fs::remove_dir_all(path)
                .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
        }
    }
    Ok(bytes)
}

fn directory_size(path: &Path) -> Result<u64, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut total = 0u64;
    for entry in
        fs::read_dir(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?
    {
        let entry =
            entry.map_err(|error| format!("cannot read entry in {}: {error}", path.display()))?;
        total += directory_size(&entry.path())?;
    }
    Ok(total)
}

fn config_path() -> PathBuf {
    gansi_home().join("config.toml")
}

fn gansi_home() -> PathBuf {
    if let Ok(home) = env::var("GANSI_HOME") {
        return PathBuf::from(home);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gansi")
}

pub(crate) fn load_global_config() -> Result<GlobalConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let config = toml::from_str(&text)
        .map_err(|error| format!("invalid config {}: {error}", path.display()))?;
    Ok(config)
}

fn save_global_config(config: &GlobalConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let payload = toml::to_string_pretty(config)
        .map_err(|error| format!("cannot serialize config: {error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("time error: {error}"))?
        .as_nanos();
    let tmp = path.with_file_name(format!(".tmp.{stamp}.toml"));
    fs::write(&tmp, payload).map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|error| format!("cannot update {}: {error}", path.display()))
}

fn read_manifest() -> Result<project::ManifestProject, String> {
    let text = fs::read_to_string(project::MANIFEST_FILE_NAME)
        .map_err(|error| format!("cannot read {}: {error}", project::MANIFEST_FILE_NAME))?;
    let manifest: project::ManifestFile =
        toml::from_str(&text).map_err(|error| format!("invalid manifest: {error}"))?;
    Ok(manifest.project)
}

fn to_option(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
const UPDATES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Updates>
  <PackageUpdate>
    <PackageName>qt.qt6.683.linux_gcc_64</PackageName>
    <Name>qt.qt6.683.gcc_64</Name>
    <Version>6.8.3-1-202406151217</Version>
    <DownloadableArchives>qtbase-6.8.3-linux-x86-offline.7z, qtdeclarative-6.8.3-linux-x86-offline.7z, qtbase-6.8.3-linux-x86-debug.7z</DownloadableArchives>
    <DownloadLocation>./packages</DownloadLocation>
    <SHA1>aabbccddeeff00112233445566778899aabbccdd</SHA1>
  </PackageUpdate>
  <PackageUpdate>
    <PackageName>qt.qt6.683.wasm_singlethread</PackageName>
    <Name>qt.qt6.683.wasm_singlethread</Name>
    <Version>6.8.3-1-202406151217</Version>
    <DownloadableArchives>qtbase-6.8.3-wasm_singlethread.7z</DownloadableArchives>
  </PackageUpdate>
</Updates>"#;

#[test]
fn finds_matching_qt_archives_in_update_xml() {
    let qt_version = QtVersion::parse("6.8.3").unwrap();
    let candidates =
        find_matching_qt_candidates(UPDATES_XML, &qt_version, "6.8.3", "gcc_64", false);

    assert!(
        candidates
            .iter()
            .any(|c| c.archive_name == "qtbase-6.8.3-linux-x86-offline.7z")
    );
    assert!(
        candidates
            .iter()
            .all(|c| c.archive_name != "qtbase-6.8.3-wasm_singlethread.7z")
    );

    let first = &candidates[0];
    assert_eq!(first.package_name, "qt.qt6.683.gcc_64");
    assert_eq!(first.package_version, "6.8.3-1-202406151217");
    assert_eq!(first.location.as_deref(), Some("./packages"));
    assert!(
        !candidates
            .iter()
            .any(|c| c.package_name != "qt.qt6.683.gcc_64")
    );
}

#[test]
fn strict_mode_filters_debug_archives() {
    let qt_version = QtVersion::parse("6.8.3").unwrap();
    let candidates = find_matching_qt_candidates(UPDATES_XML, &qt_version, "6.8.3", "gcc_64", true);

    assert!(candidates.iter().all(|c| !c.archive_name.contains("debug")));
    assert!(
        candidates
            .iter()
            .any(|c| c.archive_name.contains("qtdeclarative"))
    );
}

#[test]
fn malformed_updates_xml_yields_no_candidates() {
    let qt_version = QtVersion::parse("6.8.3").unwrap();
    let candidates = find_matching_qt_candidates(
        "<Updates><PackageUpdate><!-- broken",
        &qt_version,
        "6.8.3",
        "gcc_64",
        false,
    );
    assert!(candidates.is_empty());
}

#[cfg(test)]
const QT611_HTML: &str = r#"<html>
<a href="6.11.0-0-202603180534qtbase-Windows-Windows_11_24H2-MSVC2022-Windows-Windows_11_24H2-X86_64.7z.sha1">…</a>
<a href="6.11.0-0-202603180534qtbase-Windows-Windows_11_24H2-MSVC2022-Windows-Windows_11_24H2-X86_64.7z">…</a>
 <a href="6.11.0-0-202603180534qttools-Windows-Windows_11_24H2-MSVC2022-Windows-Windows_11_24H2-X86_64.7z">…</a>
 <a href="6.11.0-0-202603180534qtbase-linux-Rhel8.6-x86_64.7z">…</a>
 <a href="6.11.0-0-202603180534qtbase-Windows-Windows_11_24H2-MSVC2022-Windows-Windows_11_24H2-X86_64.7z.mirrorlist">…</a>
<a href="6.11.0-0-202603180534qtdoc-MacOS-MacOS_15-Clang-MacOS-MacOS_15-X86_64-ARM64.7z">…</a>
<a href="6.11.0-0-202603180534opengl32sw-64-mesa_11_2_2-signed_sha256.7z">…</a>
<a href="6.11.0-0-202603180534meta.7z">…</a>
</html>"#;

#[test]
fn lists_qt_archives_from_611_directory_html() {
    let archives = list_qt_archives(QT611_HTML);
    assert_eq!(archives.len(), 3);
    assert!(
        archives
            .iter()
            .any(|a| a.contains("Windows-Windows_11_24H2-MSVC2022"))
    );
    assert!(archives.iter().any(|a| a.contains("opengl32sw")));
    assert_eq!(archives.iter().filter(|a| a.contains("qtbase")).count(), 1);
    assert!(archives.iter().all(|a| !a.contains(".mirrorlist")
        && !a.contains(".sha1")
        && !a.contains("qtdoc")
        && a != "meta.7z"));
}

#[test]
fn extracts_module_and_checksum_from_611_names() {
    assert_eq!(
        qt_archive_module(
            "6.11.0-0-202603180534qtbase-Linux-RHEL_9_6-GCC-Linux-RHEL_9_6-X86_64.7z"
        ),
        "qtbase"
    );
    assert_eq!(
        qt_archive_module("6.11.0-0-202603180534opengl32sw-64-mesa_11_2_2-signed_sha256.7z"),
        "opengl32sw"
    );
    assert_eq!(qt_archive_module("meta.7z"), "meta");
    assert_eq!(
        parse_checksum_text(
            "aabbccddeeff00112233445566778899aabbccdd  6.11.0-0-…qtbase.7z\n",
            "sha1"
        ),
        Some(ArchiveChecksum::Sha1(
            "aabbccddeeff00112233445566778899aabbccdd".to_string()
        ))
    );
    assert_eq!(parse_checksum_text("not a hash", "sha1"), None);
}

#[test]
fn verifies_sha256_and_sha1_archive_checksums() {
    let root = test_scratch_dir("checksums");
    let archive = root.join("archive.bin");
    fs::write(&archive, b"hello").unwrap();

    let sha256 = ArchiveChecksum::Sha256(
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".to_string(),
    );
    let sha1 = ArchiveChecksum::Sha1("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".to_string());
    assert!(verify_archive_checksum(&archive, &sha256).unwrap());
    assert!(verify_archive_checksum(&archive, &sha1).unwrap());

    fs::remove_dir_all(root).unwrap();
}

#[cfg(test)]
fn test_scratch_dir(label: &str) -> std::path::PathBuf {
    let root = env::temp_dir().join(format!(
        ".gansi_test_{label}_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn extracts_7z_round_trip() {
    let root = test_scratch_dir("7z");
    let archive = root.join("sample.7z");
    {
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer.set_encrypt_header(false);
        let mut entry = sevenz_rust2::ArchiveEntry::new();
        entry.name = "dir/inner.txt".to_string();
        writer
            .push_archive_entry(entry, Some(&b"hello from 7z"[..]))
            .unwrap();
        writer.finish().unwrap();
    }
    let out = root.join("out");
    extract_7z(&archive, &out).unwrap();
    let text = fs::read_to_string(out.join("dir").join("inner.txt")).unwrap();
    assert_eq!(text, "hello from 7z");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn detects_stored_7z_symlinks() {
    let mut names = HashSet::new();
    names.insert("lib/libQt6Core.so".to_string());
    names.insert("lib/libQt6Core.so.6".to_string());
    names.insert("lib/libQt6Core.so.6.8.3".to_string());
    names.insert("bin/qmake".to_string());

    // A short sibling path is a symlink.
    assert_eq!(
        stored_symlink_target(b"libQt6Core.so.6.8.3", "lib/libQt6Core.so.6", &names),
        Some("libQt6Core.so.6.8.3")
    );
    // A real small file whose name is not in the archive is not a symlink.
    assert_eq!(
        stored_symlink_target(b"metadata", "lib/some.data", &names),
        None
    );
    // Oversized payloads cannot be links.
    let big = vec![b'x'; 1024];
    assert_eq!(stored_symlink_target(&big, "lib/big.bin", &names), None);
    // Absolute or parent-traversing targets are rejected.
    assert_eq!(stored_symlink_target(b"/etc/passwd", "lib/x", &names), None);
    assert_eq!(stored_symlink_target(b"../escape", "lib/x", &names), None);
}

#[test]
fn rejects_traversing_7z_entries() {
    let root = test_scratch_dir("7z_evil");
    let archive = root.join("evil.7z");
    {
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer.set_encrypt_header(false);
        let mut entry = sevenz_rust2::ArchiveEntry::new();
        entry.name = "../evil.txt".to_string();
        writer
            .push_archive_entry(entry, Some(&b"pwned"[..]))
            .unwrap();
        writer.finish().unwrap();
    }
    let out = root.join("out");
    assert!(extract_7z(&archive, &out).is_err());
    assert!(!out.join("..").join("evil.txt").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn accepts_matching_project_toolchain() {
    let manifest = project::ManifestProject {
        name: "smoke-app".to_string(),
        qt_version: Some("6.11.1".to_string()),
        compiler_family: Some("gcc".to_string()),
        target_arch: Some("x86_64".to_string()),
    };
    assert!(validate_toolchain_values(&manifest, Some("6.11.1"), "gcc", "x86_64").is_ok());
}

#[test]
fn reports_all_project_toolchain_mismatches() {
    let manifest = project::ManifestProject {
        name: "smoke-app".to_string(),
        qt_version: Some("6.8.3".to_string()),
        compiler_family: Some("clang".to_string()),
        target_arch: Some("aarch64".to_string()),
    };
    let error = validate_toolchain_values(&manifest, Some("6.11.1"), "gcc", "x86_64")
        .expect_err("mismatched pins must fail");
    assert!(error.contains("Qt 6.8.3 is pinned"));
    assert!(error.contains("compiler family clang is pinned"));
    assert!(error.contains("target architecture aarch64 is pinned"));
}

#[test]
fn configured_qt_root_resolves_without_path_lookup() {
    let root = test_scratch_dir("configured_qt");
    let qt_root = root.join("qt");
    let qmake = qt_root.join("bin").join(qmake_binary_name());
    fs::create_dir_all(qmake.parent().unwrap()).unwrap();
    fs::write(&qmake, b"").unwrap();
    let config = GlobalConfig {
        qt_roots: vec![qt_root.to_string_lossy().into_owned()],
        ..GlobalConfig::default()
    };

    assert_eq!(resolve_qmake(&config), Some(qmake));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn discovers_yse_checkout_from_nested_directory() {
    let root = test_scratch_dir("checkout");
    let nested = root.join("examples").join("app");
    fs::create_dir_all(root.join("crates").join("yse")).unwrap();
    fs::write(root.join("crates/yse/Cargo.toml"), b"[package]\n").unwrap();
    fs::create_dir_all(&nested).unwrap();

    assert_eq!(discover_yse_checkout(&nested), Some(root.clone()));
    assert_eq!(discover_yse_checkout(&env::temp_dir()), None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolves_configured_cargo_target_directory() {
    let cwd = Path::new("/workspace/app");
    let absolute = env::temp_dir().join("gansi-target-cache");
    assert_eq!(cargo_target_dir(cwd, None).unwrap(), cwd.join("target"));
    assert_eq!(
        cargo_target_dir(cwd, Some(std::ffi::OsStr::new("cache"))).unwrap(),
        cwd.join("cache")
    );
    assert_eq!(
        cargo_target_dir(cwd, Some(absolute.as_os_str())).unwrap(),
        absolute
    );
    assert!(cargo_target_dir(cwd, Some(std::ffi::OsStr::new(""))).is_err());
}
