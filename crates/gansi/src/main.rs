#![forbid(unsafe_code)]

//! `gansi` — the Yse developer toolchain.
//!
//! Install with `cargo install --path crates/gansi`, then:
//!
//! - `gansi create <name>` — generate a new Yse desktop application.
//! - `gansi run [--release] [-- <args>...]` — build and run the current project.
//! - `gansi test [-- <args>...]` — run the current project's tests.
//! - `gansi build [--debug] [-- <args>...]` — build and bundle the current project.
//! - `gansi doctor` — check local dependencies and Qt SDK status.

mod project;
mod template;

use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

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
        if self.major == 5 && self.minor == 9 && self.patch == 0 {
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
    /// Create a new project.
    #[command(name = "create", alias = "new")]
    Create {
        name: String,
        #[arg(long)]
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
    Setup {
        version: Option<String>,
    },

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

    /// Print toolchain version.
    Version,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Print full config.
    List,
    /// Read one value.
    Get {
        key: String,
    },
    /// Set one value (`qt_roots` accepts comma-separated paths).
    Set {
        key: String,
        value: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct GlobalConfig {
    qt_roots: Vec<String>,
    default_qt_version: Option<String>,
    compiler_family: Option<String>,
    target_arch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    project: ManifestProject,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestProject {
    name: String,
    qt_version: Option<String>,
    compiler_family: Option<String>,
    target_arch: Option<String>,
}

#[derive(Clone)]
struct HealthCheck {
    label: &'static str,
    found: bool,
    version: Option<String>,
    required: bool,
    suggestion: &'static str,
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
        Command::Doctor { verbose } => command_doctor(verbose),
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
        Command::Version => {
            println!("gansi {VERSION}");
            Ok(0)
        }
    }
}

fn command_create(name: &str, local: Option<&str>) -> Result<(), String> {
    let project = match local {
        Some(path) => project::Project::parse_local(name, path)?,
        None => project::Project::parse(name)?,
    };

    let target = env::current_dir()
        .map_err(|error| format!("cannot read current directory: {error}"))?
        .join(&project.name);
    if target.exists() {
        return Err(format!("directory `{}` already exists", target.display()));
    }
    if local.is_some() {
        project::write_project_local(&project, &target)?;
    } else {
        project::write_project(&project, &target)?;
    }
    println!(
        "Generated {} at {}\n\
         \n\
         Next steps:\n\
         \x20   cd {}\n\
         \x20   gansi run                  # build and run (requires Qt 6 development files)\n\
         \x20   gansi test                 # run tests\n\
         \x20   gansi build                # build a release bundle",
        project.name,
        target.display(),
        project.name
    );
    Ok(())
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

fn command_doctor(verbose: bool) -> Result<i32, String> {
    let checks = collect_health_checks()?;
    render_checks(&checks);
    if verbose {
        render_verbose_environment()?;
    }

    if checks.iter().any(|check| check.required && !check.found) {
        Ok(2)
    } else if checks.iter().any(|check| !check.found) {
        Ok(1)
    } else {
        Ok(0)
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
        .unwrap_or_else(|| default_arch());
    let platform = detect_platform();
    let expected_root = gansi_home()
        .join("qt")
        .join(&qt_version)
        .join(platform)
        .join(&arch);

    if has_qmake(&expected_root) {
        config.default_qt_version = Some(qt_version.clone());
        if !config
            .qt_roots
            .iter()
            .any(|value| Path::new(value) == expected_root.as_path())
        {
            config.qt_roots.push(expected_root.to_string_lossy().into_owned());
        }
        save_global_config(&config)?;
        println!("Detected existing Qt layout at {}", expected_root.display());
        println!("Registered {} in config.", expected_root.display());
        return Ok(0);
    }

    if let Some(root) = config
        .qt_roots
        .iter()
        .find_map(|value| {
            let path = Path::new(value);
            if has_qmake(path) {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
    {
        println!("Using existing Qt root: {}", root.display());
        config.default_qt_version = Some(qt_version.clone());
        save_global_config(&config)?;
        return Ok(0);
    }

    if let Some(qmake) = locate_qmake() {
        let root = qmake
            .parent()
            .and_then(|bin| bin.parent())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        println!("Found Qt via PATH: {}", root.display());
        config.default_qt_version = Some(qt_version);
        if !config
            .qt_roots
            .iter()
            .any(|value| Path::new(value) == root.as_path())
        {
            config.qt_roots.push(root.to_string_lossy().into_owned());
        }
        save_global_config(&config)?;
        println!("Registered {} in config.", root.display());
        Ok(0)
    } else if let Some(root) = install_qt_from_mirrors(&qt_version, &platform, &arch, &expected_root)? {
        config.default_qt_version = Some(qt_version);
        if !config
            .qt_roots
            .iter()
            .any(|value| Path::new(value) == root.as_path())
        {
            config.qt_roots.push(root.to_string_lossy().into_owned());
        }
        save_global_config(&config)?;
        println!("Installed and registered Qt: {}", root.display());
        Ok(0)
    } else {
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
}

fn install_qt_from_mirrors(
    qt_version: &str,
    platform: &str,
    requested_arch: &str,
    expected_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let requested_version = qt_version;
    if expected_root.exists() && !has_qmake(expected_root) {
        fs::remove_dir_all(expected_root).map_err(|error| {
            format!("cannot clean stale Qt root {}: {error}", expected_root.display())
        })?;
    }

    let qt_version = QtVersion::parse(requested_version)?;
    let version_short = qt_version.short();
    let repo_os_arch = qt_os_arch(platform);
    let qt_arch = qt_archive_arch(platform, requested_arch, &qt_version);
    let mut install_root: Option<PathBuf> = None;
    let qt_full_version = format!("{}.{}.{}", qt_version.major, qt_version.minor, qt_version.patch);

    let base_folder = format!("qt{}_{}", qt_version.major, version_short);
    let use_split = qt_version.major > 6 || (qt_version.major == 6 && qt_version.minor >= 8);
    let version_path = if use_split {
        format!("{0}/{0}", base_folder)
    } else {
        base_folder.clone()
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
            &[
                mirror.root_path,
                repo_os_arch,
                QT_TARGET,
                &version_path,
            ],
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
        for candidate in candidates {
            let mut archive_path = String::new();
            archive_path.push_str(candidate.package_name.as_str());
            if !archive_path.is_empty() {
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

            let archive_name = candidate.archive_name;
            let archive_url = url_join(&mirror_base, &[&archive_path]);

            let stage = work_dir.join(format!(
                "archive_{}",
                archive_name.replace('/', "_").replace('\\', "_")
            ));
            match http_download_file(&archive_url, &stage) {
                Ok(()) => {}
                Err(error) => {
                    println!("  {} download failed: {error}", archive_name);
                    continue;
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
                println!("  failed to extract {}: {error}", archive_name);
                let _ = fs::remove_file(&stage);
                continue;
            }
            let found_root = find_qmake_root(&extract_dir);
            if let Some(found_root) = found_root {
                if has_qmake(&found_root) {
                    if found_root != *expected_root {
                        if expected_root.exists() {
                            fs::remove_dir_all(expected_root).map_err(|error| {
                                format!("cannot clear {}: {error}", expected_root.display())
                            })?;
                        }
                        copy_dir_all(&found_root, expected_root)?;
                        install_root = Some(expected_root.to_path_buf());
                    } else {
                        install_root = Some(found_root);
                    }
                }
        } else if let Some(payload_root) = find_qt_payload_root(&extract_dir, &qt_full_version, &qt_arch) {
                if let Some(root) = install_root.as_deref() {
                    if root.exists() {
                        if is_top_level_library_payload(&payload_root) {
                            copy_dir_all(&payload_root, &root.join("lib"))?;
                        } else {
                            copy_dir_all(&payload_root, root)?;
                        }
                    }
                }
            }
        }

        if install_root.is_some() {
            break;
        }
    }

    if let Some(root) = install_root.as_deref() {
        ensure_qt_library_soname_links(root)?;
    }

    if let Err(error) = fs::remove_dir_all(&work_dir) {
        println!("warning: cannot remove temp dir {}: {error}", work_dir.display());
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

#[derive(Clone)]
struct QtArchiveCandidate {
    archive_name: String,
    package_version: String,
    package_name: String,
    location: Option<String>,
}

fn find_matching_qt_candidates(
    updates_xml: &str,
    qt_version: &QtVersion,
    requested_version: &str,
    qt_arch: &str,
    strict: bool,
) -> Vec<QtArchiveCandidate> {
    let mut candidates = Vec::new();
    let mut cursor = updates_xml;
    let marker = "</PackageUpdate>";
    let mut seen = HashSet::new();

    while let Some(start) = cursor.find("<PackageUpdate") {
        let block = match cursor[start..].find(marker) {
            Some(end_of_package) => &cursor[start..start + end_of_package + marker.len()],
            None => break,
        };
        cursor = &cursor[start + block.len()..];

        let Some(name) = extract_xml_text(block, "Name") else {
            continue;
        };
        let version = extract_xml_text(block, "Version").unwrap_or_default();
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
        let Some(downloads) = extract_xml_text(block, "DownloadableArchives") else {
            continue;
        };
        let location = extract_xml_text(block, "DownloadLocation").filter(|value| !value.is_empty());
        let is_base_package = is_qt_base_package(&name, qt_version);
        for archive in split_archives(&downloads) {
            if strict {
                if !is_base_package || is_debug_archive(&archive) {
                    continue;
                }
            } else if !is_base_package && !is_qtbase_archive(&archive) {
                continue;
            }
            let key = if let Some(location) = location.as_deref() {
                format!("{name}|{location}|{version}|{archive}")
            } else {
                format!("{name}|{version}|{archive}")
            };
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
    if let Some(package_version) = package_version {
        if qt_version_matches(package_version, qt_version) {
            return true;
        }
    }
    version_token_matches(&name, requested_version, qt_version)
}

fn qt_version_matches(value: &str, qt_version: &QtVersion) -> bool {
    let value = value.split_once('-').map_or(value, |(left, _)| left);
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let major = match parts[0].parse::<u32>() {
        Ok(major) => major,
        Err(_) => return false,
    };
    let minor = match parts[1].parse::<u32>() {
        Ok(minor) => minor,
        Err(_) => return false,
    };
    if major != qt_version.major || minor != qt_version.minor {
        return false;
    }
    if qt_version.patch == 0 {
        return true;
    }
    let patch = match parts.get(2).and_then(|value| value.parse::<u32>().ok()) {
        Some(patch) => patch,
        None => return true,
    };
    patch == qt_version.patch
}

fn version_token_matches(name: &str, requested_version: &str, qt_version: &QtVersion) -> bool {
    let requested = requested_version.to_lowercase();
    let requested_with_underscore = requested.replace('.', "_");
    let candidates = vec![
        requested,
        requested_with_underscore,
        qt_version.short(),
        format!("{}{}", qt_version.major, qt_version.minor),
        format!("{}_{}", qt_version.major, qt_version.minor),
        format!("{}.{}", qt_version.major, qt_version.minor),
        format!(
            "{}.{}.{}",
            qt_version.major,
            qt_version.minor,
            qt_version.patch
        ),
        format!(
            "{}_{}_{}",
            qt_version.major,
            qt_version.minor,
            qt_version.patch
        ),
    ];
    candidates.iter().any(|candidate| name.contains(candidate))
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
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_string())
        .collect()
}

fn archive_preference(name: &str) -> u8 {
    let name = name.to_lowercase();
    if name.ends_with(".7z") {
        0
    } else if name.ends_with(".zip") {
        1
    } else if name.ends_with(".tar.xz") || name.ends_with(".tar.bz2") || name.ends_with(".tar.gz") {
        2
    } else if name.ends_with(".tar") {
        3
    } else {
        4
    }
}

fn extract_xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let rest = &xml[start + open.len()..];
    let end = rest.find(&close)?;
    Some(rest[..end].trim().to_string())
}

fn http_get_text(url: &str) -> Result<String, String> {
    if find_command("curl").is_some() {
        run_command_output("curl", &["-L", "--fail", "--silent", "--show-error", url])
    } else if find_command("wget").is_some() {
        run_command_output("wget", &["-qO-", "-t", "3", url])
    } else {
        Err("curl or wget is required to download Qt metadata".to_string())
    }
}

fn http_download_file(url: &str, target: &Path) -> Result<(), String> {
    if find_command("curl").is_some() {
        run_command("curl", &["-L", "--fail", "--silent", "--show-error", "-o", &target.to_string_lossy(), url])
    } else if find_command("wget").is_some() {
        run_command("wget", &["-q", "-O", &target.to_string_lossy(), "-t", "3", url])
    } else {
        Err("curl or wget is required to download Qt archives".to_string())
    }
}

fn run_command_output(command: &str, args: &[&str]) -> Result<String, String> {
    let output = ProcessCommand::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run `{command}`: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(format!(
            "`{command}` failed (exit {}): {} {}",
            output.status.code().unwrap_or(-1),
            if !stdout.is_empty() { format!("stdout={stdout}") } else { String::new() },
            if !stderr.is_empty() { format!("stderr={stderr}") } else { String::new() },
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
    let status = ProcessCommand::new(command)
        .args(args)
        .status()
        .map_err(|error| format!("cannot run `{command}`: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{command}` failed with status {:?}", status.code()))
    }
}

fn extract_archive(archive: &Path, target: &Path) -> Result<(), String> {
    let archive_name = archive
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let archive_name = archive_name.to_lowercase();
    fs::create_dir_all(target).map_err(|error| {
        format!("cannot create extraction dir {}: {error}", target.display())
    })?;

    if archive_name.ends_with(".7z") {
        if find_command("7z").is_some() {
            run_command(
                "7z",
                &[
                    "x",
                    "-y",
                    "-aoa",
                    "-snh-",
                    "-snl-",
                    "-bd",
                    &format!("-o{}", target.to_string_lossy()),
                    &archive.to_string_lossy(),
                ],
            )
        } else if find_command("7za").is_some() {
            run_command(
                "7za",
                &[
                    "x",
                    "-y",
                    "-aoa",
                    "-bd",
                    &format!("-o{}", target.to_string_lossy()),
                    &archive.to_string_lossy(),
                ],
            )
        } else {
            Err("7z/7za is required to extract .7z archive".into())
        }
    } else if archive_name.ends_with(".zip") {
        if find_command("unzip").is_some() {
            run_command(
                "unzip",
                &["-q", "-o", &archive.to_string_lossy(), "-d", &target.to_string_lossy()],
            )
        } else if find_command("tar").is_some() {
            run_command(
                "tar",
                &[
                    "-xf",
                    &archive.to_string_lossy(),
                    "-C",
                    &target.to_string_lossy(),
                ],
            )
        } else {
            Err("unzip or tar is required to extract .zip archive".into())
        }
    } else if archive_name.ends_with(".tar.xz")
        || archive_name.ends_with(".tar.gz")
        || archive_name.ends_with(".tar.bz2")
        || archive_name.ends_with(".tar")
    {
        if find_command("tar").is_some() {
            run_command(
                "tar",
                &[
                    "-xf",
                    &archive.to_string_lossy(),
                    "-C",
                    &target.to_string_lossy(),
                ],
            )
        } else {
            Err("tar is required to extract .tar archive".into())
        }
    } else {
        Err(format!("unsupported archive format: {archive_name}"))
    }
}

#[cfg(unix)]
fn ensure_qt_library_soname_links(root: &Path) -> Result<(), String> {
    for dirname in ["lib", "lib64"] {
        let dir = root.join(dirname);
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|error| {
            format!("cannot list {dir}: {error}", dir = dir.display())
        })? {
            let entry = entry.map_err(|error| format!("cannot read {dir}: {error}", dir = dir.display()))?;
            if !entry.file_type().map_err(|error| {
                format!(
                    "cannot read file type {}: {error}",
                    entry.path().display()
                )
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
            std::os::unix::fs::symlink(
                entry.file_name(),
                &link,
            )
            .map_err(|error| {
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
            if let Ok(metadata) = entry_path.symlink_metadata() {
                if metadata.is_file() || metadata.is_symlink() {
                    continue;
                }
            }
            if entry.file_type().ok().map_or(false, |value| value.is_dir()) {
                stack.push(entry_path);
            }
        }
    }
    None
}

fn find_qt_payload_root(
    extract_dir: &Path,
    qt_version: &str,
    qt_arch: &str,
) -> Option<PathBuf> {
    let version_root = extract_dir.join(qt_version);
    if version_root.exists() {
        let qt_root = version_root.join(qt_arch);
        if qt_root.exists()
            && (qt_root.join("bin").is_dir() || qt_root.join("lib").is_dir())
        {
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
            if let Some(name) = entry_path.file_name().and_then(|value| value.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
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
        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => return false,
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
            fs::create_dir_all(parent).map_err(|error| {
                format!("cannot create {}: {error}", parent.display())
            })?;
        }
        if dst.exists() {
            fs::remove_file(dst).map_err(|error| {
                format!("cannot replace existing file {}: {error}", dst.display())
            })?;
        }
        fs::copy(src, dst).map_err(|error| {
            format!("cannot copy {} to {}: {error}", src.display(), dst.display())
        })?;
        return Ok(());
    }
    if !dst.exists() {
        fs::create_dir_all(dst)
            .map_err(|error| format!("cannot create {}: {error}", dst.display()))?;
    }
    for entry in fs::read_dir(src).map_err(|error| {
        format!("cannot read source directory {}: {error}", src.display())
    })? {
        let entry = entry
            .map_err(|error| format!("cannot read source entry in {}: {error}", src.display()))?;
        let source = entry.path();
        let dest = dst.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            format!(
                "cannot read metadata for source path {}: {error}",
                source.display()
            )
        })?;
        if metadata.is_dir() {
            if !dest.exists() {
                fs::create_dir_all(&dest).map_err(|error| {
                    format!("cannot create {}: {error}", dest.display())
                })?;
            }
            copy_dir_all(&source, &dest)?;
        } else {
            if dest.exists() {
                let _ = fs::remove_file(&dest);
            }
            fs::copy(&source, &dest)
                .map_err(|error| format!("cannot copy {} to {}: {error}", source.display(), dest.display()))?;
        }
    }
    Ok(())
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

fn qt_os_arch(platform: &str) -> &'static str {
    if platform == "windows" {
        "windows_x86"
    } else if platform == "macos" {
        "mac_x64"
    } else {
        "linux_x64"
    }
}

fn qt_archive_arch(platform: &str, requested_arch: &str, qt_version: &QtVersion) -> String {
    if platform == "windows" {
        if qt_version.major == 6 && qt_version.minor >= 8 || qt_version.major > 6 {
            "win64_msvc2022_64".to_string()
        } else {
            "win64_msvc2019_64".to_string()
        }
    } else if platform == "macos" {
        "clang_64".to_string()
    } else if requested_arch == "aarch64" {
        "gcc_arm64".to_string()
    } else {
        "gcc_64".to_string()
    }
}

fn command_clean() -> Result<(), String> {
    let cwd = env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let reclaimed = reclaim_targets(&[cwd.join("target"), cwd.join("dist")])?;
    println!("Reclaimed approximately {reclaimed} bytes");
    Ok(())
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
                    config.qt_roots = value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string())
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect();
                }
                "default_qt_version" => {
                    config.default_qt_version = to_option(value);
                }
                "compiler_family" => {
                    config.compiler_family = to_option(value);
                }
                "target_arch" => {
                    config.target_arch = to_option(value);
                }
                _ => return Err(format!("unknown key `{key}`")),
            };
            save_global_config(&config)?;
            println!("Updated.");
            Ok(0)
        }
    }
}

fn command_init() -> Result<(), String> {
    let cwd = env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let cwd_name = cwd
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("invalid current directory name")?;
    let project = project::Project::parse(cwd_name)?;

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
        2 => Err("required environment checks failed. Run `gansi doctor`.".into()),
        1 => {
            eprintln!("Warning: environment has soft-missing items. Build may still work.");
            Ok(())
        }
        0 => Ok(()),
        _ => Err("environment checks failed".into()),
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

    let mut roots: Vec<PathBuf> = config
        .qt_roots
        .iter()
        .map(PathBuf::from)
        .filter(|path| has_qmake(path))
        .collect();
        if let Some(path) = env::var_os("QMAKE").map(PathBuf::from) {
            let root = path
                .parent()
                .and_then(|value| value.parent())
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            if has_qmake(&root) {
                roots.push(root);
            }
    }
    if roots.is_empty() {
        if let Some(root) = locate_qmake().and_then(|path| {
            path.parent()
                .and_then(|value| value.parent())
                .map(|value| value.to_path_buf())
                .filter(|root| has_qmake(root))
        }) {
            roots.push(root);
        }
    }

    if let Some(root) = roots.into_iter().next() {
        let qmake = root.join("bin").join(qmake_binary_name());
        let path = env::var_os("PATH").unwrap_or_else(OsString::new);
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
            let mut entries = env::split_paths(&env::var_os("LD_LIBRARY_PATH").unwrap_or_default()).collect::<Vec<_>>();
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

fn collect_health_checks() -> Result<Vec<HealthCheck>, String> {
    let mut checks = vec![
        check_tool("rustc", &["--version"], true, "Install Rust via rustup."),
        check_tool("cargo", &["--version"], true, "Install Rust via rustup."),
        check_tool("cmake", &["--version"], false, "Install CMake."),
    ];
    if let Some(qmake) = locate_qmake() {
        checks.push(HealthCheck {
            label: "qmake",
            found: true,
            version: qmake_version(&qmake),
            required: false,
            suggestion: "n/a",
        });
    } else {
        checks.push(HealthCheck {
            label: "qmake",
            found: false,
            version: None,
            required: false,
            suggestion: "Install Qt 6 (qmake).",
        });
    }
    checks.push(check_tool(
        "pkg-config",
        &["--version"],
        false,
        "Install pkg-config.",
    ));

    Ok(checks)
}

fn check_tool(
    command: &'static str,
    args: &[&str],
    required: bool,
    suggestion: &'static str,
) -> HealthCheck {
    if let Some(version) = tool_version(command, args) {
        HealthCheck {
            label: command,
            found: true,
            version: Some(version),
            required,
            suggestion,
        }
    } else {
        HealthCheck {
            label: command,
            found: false,
            version: None,
            required,
            suggestion,
        }
    }
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
    let output = ProcessCommand::new(path).arg("-v").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| line.split("Qt version ").nth(1).map(|v| v.to_string()))
        .or_else(|| text.lines().next().map(|line| line.to_string()))
}

fn tool_version(command: &str, args: &[&str]) -> Option<String> {
    let output = ProcessCommand::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_string())
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

fn qmake_binary_name() -> &'static str {
    if cfg!(windows) {
        "qmake.exe"
    } else {
        "qmake"
    }
}

fn detect_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
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
            fs::remove_dir_all(path).map_err(|error| {
                format!("cannot remove {}: {error}", path.display())
            })?;
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
    for entry in fs::read_dir(path).map_err(|error| format!("cannot open {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("cannot read entry in {}: {error}", path.display()))?;
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
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("gansi")
}

fn load_global_config() -> Result<GlobalConfig, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    let text = fs::read_to_string(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let config = toml::from_str(&text).map_err(|error| format!("invalid config {}: {error}", path.display()))?;
    Ok(config)
}

fn save_global_config(config: &GlobalConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let payload = toml::to_string_pretty(config).map_err(|error| format!("cannot serialize config: {error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("time error: {error}"))?
        .as_nanos();
    let tmp = path.with_file_name(format!(".tmp.{stamp}.toml"));
    fs::write(&tmp, payload).map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|error| format!("cannot update {}: {error}", path.display()))
}

fn read_manifest() -> Result<ManifestProject, String> {
    let text = fs::read_to_string(project::MANIFEST_FILE_NAME)
        .map_err(|error| format!("cannot read {}: {error}", project::MANIFEST_FILE_NAME))?;
    let manifest: ManifestFile = toml::from_str(&text).map_err(|error| format!("invalid manifest: {error}"))?;
    Ok(manifest.project)
}

fn to_option(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
