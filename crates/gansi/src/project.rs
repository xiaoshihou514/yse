//! Project model, name handling, and bundle creation.

use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
#[cfg(target_os = "linux")]
use sha2::{Digest, Sha256};

use crate::template;

pub const MANIFEST_FILE_NAME: &str = "gansi.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestFile {
    pub project: ManifestProject,
    #[serde(default)]
    pub deployment: ManifestDeployment,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestProject {
    pub name: String,
    pub qt_version: Option<String>,
    pub compiler_family: Option<String>,
    pub target_arch: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ManifestDeployment {
    pub bundle_name: Option<String>,
    pub app_id: Option<String>,
}

#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
    license: Option<String>,
    license_file: Option<PathBuf>,
    repository: Option<String>,
    manifest_path: PathBuf,
}

pub struct Project {
    pub name: String,
    pub name_snake: String,
    pub title: String,
    pub app_id: String,
    pub local_yse: Option<PathBuf>,
    pub qt_version: String,
    pub compiler_family: String,
    pub target_arch: String,
}

#[derive(Debug, Clone, Copy)]
pub enum BundleProfile {
    Debug,
    Release,
}

impl BundleProfile {
    const fn target_dir(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

impl Project {
    /// Validate a project name and derive its variants.
    pub fn parse(name: &str) -> Result<Self, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("project name must not be empty".into());
        }
        if trimmed.len() > 64 {
            return Err("project name must be at most 64 characters".into());
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(
                "project name may only contain lowercase letters, digits, '-' and '_'".into(),
            );
        }
        if trimmed.starts_with('-') || trimmed.starts_with('_') || trimmed.ends_with('-') {
            return Err("project name must not start with '-'/'_' or end with '-'".into());
        }
        Ok(Self {
            name: trimmed.to_string(),
            name_snake: trimmed.replace('-', "_"),
            title: title_case(trimmed),
            app_id: format!("com.example.{}", trimmed.replace('_', "-")),
            local_yse: None,
            qt_version: "6".to_string(),
            compiler_family: default_compiler_family().to_string(),
            target_arch: env::consts::ARCH.to_string(),
        })
    }

    /// Record the toolchain detected while creating the project.
    pub fn pin_toolchain(
        &mut self,
        qt_version: impl Into<String>,
        compiler_family: impl Into<String>,
        target_arch: impl Into<String>,
    ) {
        self.qt_version = qt_version.into();
        self.compiler_family = compiler_family.into();
        self.target_arch = target_arch.into();
    }

    /// Parse a project name and pin it to a local Yse repository checkout.
    /// The pinned path feeds `cargo add yse --path <checkout>/crates/yse`.
    pub fn parse_local(name: &str, yse_path: &str) -> Result<Self, String> {
        let mut project = Self::parse(name)?;
        let absolute = std::path::Path::new(yse_path)
            .canonicalize()
            .map_err(|error| format!("cannot resolve yse path `{yse_path}`: {error}"))?;
        if !absolute.join("crates").join("yse").is_dir() {
            return Err(format!(
                "`{}` does not contain crates/yse; point at the yse repository root",
                absolute.display()
            ));
        }
        project.local_yse = Some(absolute);
        Ok(project)
    }

    /// Read the pinned project configuration from `gansi.toml`.
    pub fn from_manifest(path: &str) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("cannot read `{path}` (run `gansi create` first): {error}"))?;
        let manifest: ManifestFile = toml::from_str(&content)
            .map_err(|error| format!("`{path}` is not a valid gansi manifest: {error}"))?;
        let mut project = Self::parse(&manifest.project.name)?;
        if let Some(bundle_name) = manifest.deployment.bundle_name {
            let bundle_name = bundle_name.trim();
            if bundle_name.is_empty()
                || bundle_name.len() > 128
                || bundle_name.chars().any(char::is_control)
                || bundle_name.contains(['/', '\\'])
            {
                return Err(
                    "deployment.bundle_name must contain 1-128 printable characters without path separators"
                        .into(),
                );
            }
            project.title = bundle_name.to_string();
        }
        if let Some(app_id) = manifest.deployment.app_id {
            validate_app_id(&app_id)?;
            project.app_id = app_id;
        }
        Ok(project)
    }
}

fn validate_app_id(app_id: &str) -> Result<(), String> {
    let parts: Vec<_> = app_id.split('.').collect();
    let valid = parts.len() >= 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
                && part
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_alphanumeric())
                && part
                    .chars()
                    .last()
                    .is_some_and(|character| character.is_ascii_alphanumeric())
        });
    if valid && app_id.len() <= 255 {
        Ok(())
    } else {
        Err(format!(
            "deployment.app_id `{app_id}` must be a reverse-DNS ID with at least three dot-separated ASCII components"
        ))
    }
}

const fn default_compiler_family() -> &'static str {
    if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        "gcc"
    }
}

fn title_case(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

fn xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(any(target_os = "macos", test))]
fn macos_info_plist(project: &Project) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleExecutable</key><string>{}</string><key>CFBundleIdentifier</key><string>{}</string><key>CFBundleName</key><string>{}</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
        xml_text(&project.name),
        xml_text(&project.app_id),
        xml_text(&project.title),
    )
}

pub fn write_project(project: &Project, target: &Path) -> Result<(), String> {
    let files: Vec<(&str, String)> = vec![
        (
            "Cargo.toml",
            template::render(template::CARGO_TOML, project),
        ),
        ("build.rs", template::render(template::BUILD_RS, project)),
        (
            "gansi.toml",
            template::render(template::GANSI_TOML, project),
        ),
        (".gitignore", template::render(template::GITIGNORE, project)),
        ("LICENSE-MIT", template::LICENSE_MIT.to_string()),
        ("LICENSE-APACHE", template::LICENSE_APACHE.to_string()),
        (
            ".github/workflows/ci.yml",
            template::render(template::GITHUB_CI_YML, project),
        ),
        (
            "resources/linux/application.desktop",
            template::render(template::LINUX_DESKTOP, project),
        ),
        (
            "resources/linux/application.metainfo.xml",
            template::render(template::LINUX_APPSTREAM, project),
        ),
        (
            "resources/linux/application.svg",
            template::render(template::APPLICATION_SVG, project),
        ),
        (
            "RELEASE.md",
            template::render(template::RELEASE_MD, project),
        ),
        ("README.md", template::render(template::README_MD, project)),
        (
            "THIRD_PARTY_NOTICES.txt",
            template::render(template::THIRD_PARTY_NOTICES, project),
        ),
        ("src/main.rs", template::render(template::MAIN_RS, project)),
        (
            "tests/smoke.rs",
            template::render(template::TESTS_SMOKE_RS, project),
        ),
    ];
    for (relative, content) in files {
        let path = target.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        fs::write(&path, content)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Build a release and lay out `dist/<name>/` with the platform binary.
/// Linux bundles the Qt runtime behind a launcher; Windows and macOS invoke
/// Qt's deployment tools.
pub fn bundle(project: &Project, profile: BundleProfile) -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|error| format!("cannot read current directory: {error}"))?;
    let dist_root = current_dir.join("dist");
    let final_dist = dist_root.join(&project.name);
    let dist = dist_root.join(format!(".{}.staging", project.name));
    if dist.exists() {
        fs::remove_dir_all(&dist)
            .map_err(|error| format!("cannot clean {}: {error}", dist.display()))?;
    }
    fs::create_dir_all(&dist)
        .map_err(|error| format!("cannot create {}: {error}", dist.display()))?;

    let binary_name = format!("{}{}", project.name, env::consts::EXE_SUFFIX);
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                current_dir.join(path)
            }
        })
        .unwrap_or_else(|| current_dir.join("target"));
    let binary = target_dir.join(profile.target_dir()).join(binary_name);
    #[cfg(target_os = "linux")]
    let destination = {
        let libexec = dist.join("libexec");
        fs::create_dir_all(&libexec)
            .map_err(|error| format!("cannot create {}: {error}", libexec.display()))?;
        libexec.join(&project.name)
    };

    #[cfg(all(not(target_os = "linux"), not(target_os = "macos")))]
    let destination = dist.join(format!("{}{}", project.name, env::consts::EXE_SUFFIX));

    #[cfg(target_os = "macos")]
    let destination = {
        let app = dist.join(format!("{}.app", project.title));
        let macos = app.join("Contents").join("MacOS");
        fs::create_dir_all(&macos)
            .map_err(|error| format!("cannot create {}: {error}", macos.display()))?;
        fs::write(
            app.join("Contents").join("Info.plist"),
            macos_info_plist(project),
        )
        .map_err(|error| format!("cannot write macOS bundle metadata: {error}"))?;
        macos.join(&project.name)
    };
    fs::copy(&binary, &destination)
        .map_err(|error| format!("cannot copy binary {}: {error}", binary.display()))?;
    let notices = current_dir.join("THIRD_PARTY_NOTICES.txt");
    fs::copy(&notices, dist.join("THIRD_PARTY_NOTICES.txt")).map_err(|error| {
        format!(
            "cannot copy third-party notices {}: {error}",
            notices.display()
        )
    })?;
    write_cargo_license_inventory(&current_dir, &dist)?;
    let rust_licenses = dist.join("licenses/rust");
    fs::create_dir_all(&rust_licenses).map_err(|error| {
        format!(
            "cannot create Rust license directory {}: {error}",
            rust_licenses.display()
        )
    })?;
    fs::write(rust_licenses.join("LICENSE-MIT"), template::LICENSE_MIT)
        .map_err(|error| format!("cannot write bundled MIT license: {error}"))?;
    fs::write(
        rust_licenses.join("LICENSE-APACHE"),
        template::LICENSE_APACHE,
    )
    .map_err(|error| format!("cannot write bundled Apache license: {error}"))?;

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let qt_platform = env::var("QT_QPA_PLATFORM").unwrap_or_default();

    #[cfg(target_os = "linux")]
    {
        deploy_linux_desktop_assets(project, &current_dir, &dist)?;
        deploy_linux_qt_runtime(project, &destination, &dist)?;
    }

    #[cfg(target_os = "windows")]
    if qt_platform != "offscreen" {
        run_deploy_tool(
            "windeployqt",
            &[destination.to_string_lossy().to_string()],
            &dist,
        )?;
    }
    #[cfg(target_os = "macos")]
    if qt_platform != "offscreen" {
        run_deploy_tool(
            "macdeployqt",
            &[dist
                .join(format!("{}.app", project.title))
                .to_string_lossy()
                .to_string()],
            &dist,
        )?;
    }

    if final_dist.exists() {
        fs::remove_dir_all(&final_dist)
            .map_err(|error| format!("cannot replace {}: {error}", final_dist.display()))?;
    }
    fs::rename(&dist, &final_dist).map_err(|error| {
        format!(
            "cannot publish bundle {} as {}: {error}",
            dist.display(),
            final_dist.display()
        )
    })?;

    #[cfg(target_os = "linux")]
    create_linux_archive(project, &final_dist, &dist_root)?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn create_linux_archive(project: &Project, bundle: &Path, dist_root: &Path) -> Result<(), String> {
    use flate2::Compression;
    use flate2::GzBuilder;

    let archive_name = format!("{}-linux-{}.tar.gz", project.name, env::consts::ARCH);
    let destination = dist_root.join(&archive_name);
    let staging = dist_root.join(format!(".{archive_name}.staging"));
    if staging.exists() {
        fs::remove_file(&staging)
            .map_err(|error| format!("cannot clean {}: {error}", staging.display()))?;
    }
    let output = fs::File::create(&staging)
        .map_err(|error| format!("cannot create {}: {error}", staging.display()))?;
    let epoch = source_date_epoch()?;
    let gzip_epoch = u32::try_from(epoch)
        .map_err(|_| format!("SOURCE_DATE_EPOCH {epoch} exceeds the gzip timestamp range"))?;
    let encoder = GzBuilder::new()
        .mtime(gzip_epoch)
        .write(output, Compression::default());
    let mut archive = tar::Builder::new(encoder);
    append_reproducible_bundle(&mut archive, &project.name, bundle, epoch)?;
    let encoder = archive
        .into_inner()
        .map_err(|error| format!("cannot finish tar archive: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("cannot finish gzip archive: {error}"))?;
    if destination.exists() {
        fs::remove_file(&destination)
            .map_err(|error| format!("cannot replace {}: {error}", destination.display()))?;
    }
    fs::rename(&staging, &destination).map_err(|error| {
        format!(
            "cannot publish archive {} as {}: {error}",
            staging.display(),
            destination.display()
        )
    })?;
    write_sha256_checksum(&destination, dist_root)
}

#[cfg(target_os = "linux")]
fn source_date_epoch() -> Result<u64, String> {
    match env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("SOURCE_DATE_EPOCH `{value}` must be an unsigned integer")),
        Err(env::VarError::NotPresent) => Ok(0),
        Err(error) => Err(format!("cannot read SOURCE_DATE_EPOCH: {error}")),
    }
}

#[cfg(target_os = "linux")]
fn append_reproducible_bundle<W: std::io::Write>(
    archive: &mut tar::Builder<W>,
    archive_root: &str,
    bundle: &Path,
    epoch: u64,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut entries = Vec::new();
    let mut directories = vec![bundle.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        {
            let path = entry
                .map_err(|error| format!("cannot read archive entry: {error}"))?
                .path();
            if path.is_dir() {
                directories.push(path.clone());
            }
            entries.push(path);
        }
    }
    entries.sort_by(|left, right| {
        left.strip_prefix(bundle)
            .unwrap_or(left)
            .cmp(right.strip_prefix(bundle).unwrap_or(right))
    });

    append_tar_directory(archive, Path::new(archive_root), epoch)?;
    for source in entries {
        let relative = source
            .strip_prefix(bundle)
            .map_err(|error| format!("cannot relativize {}: {error}", source.display()))?;
        let destination = Path::new(archive_root).join(relative);
        let metadata = fs::metadata(&source)
            .map_err(|error| format!("cannot inspect {}: {error}", source.display()))?;
        if metadata.is_dir() {
            append_tar_directory(archive, &destination, epoch)?;
        } else if metadata.is_file() {
            let mode = if metadata.permissions().mode() & 0o111 == 0 {
                0o644
            } else {
                0o755
            };
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(metadata.len());
            header.set_mode(mode);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(epoch);
            header.set_cksum();
            let mut file = fs::File::open(&source)
                .map_err(|error| format!("cannot open {}: {error}", source.display()))?;
            archive
                .append_data(&mut header, &destination, &mut file)
                .map_err(|error| format!("cannot archive {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn append_tar_directory<W: std::io::Write>(
    archive: &mut tar::Builder<W>,
    path: &Path,
    epoch: u64,
) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_size(0);
    header.set_mode(0o755);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(epoch);
    header.set_cksum();
    archive
        .append_data(&mut header, path, std::io::empty())
        .map_err(|error| format!("cannot archive directory {}: {error}", path.display()))
}

#[cfg(target_os = "linux")]
fn write_sha256_checksum(artifact: &Path, dist_root: &Path) -> Result<(), String> {
    use std::io::Read;

    let mut file = fs::File::open(artifact)
        .map_err(|error| format!("cannot open {} for hashing: {error}", artifact.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot hash {}: {error}", artifact.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let file_name = artifact
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid artifact name {}", artifact.display()))?;
    fs::write(
        dist_root.join("SHA256SUMS"),
        format!("{:x}  {file_name}\n", hasher.finalize()),
    )
    .map_err(|error| format!("cannot write SHA256SUMS: {error}"))
}

#[cfg(target_os = "linux")]
fn deploy_linux_desktop_assets(
    project: &Project,
    project_dir: &Path,
    dist: &Path,
) -> Result<(), String> {
    let source = project_dir.join("resources/linux");
    let applications = dist.join("share/applications");
    let icons = dist.join("share/icons/hicolor/scalable/apps");
    let metainfo = dist.join("share/metainfo");
    fs::create_dir_all(&applications)
        .map_err(|error| format!("cannot create {}: {error}", applications.display()))?;
    fs::create_dir_all(&icons)
        .map_err(|error| format!("cannot create {}: {error}", icons.display()))?;
    fs::create_dir_all(&metainfo)
        .map_err(|error| format!("cannot create {}: {error}", metainfo.display()))?;
    let desktop_source = source.join("application.desktop");
    let desktop = fs::read_to_string(&desktop_source).map_err(|error| {
        format!(
            "cannot read Linux desktop entry {}: {error}",
            desktop_source.display()
        )
    })?;
    let desktop = rewrite_desktop_identity(&desktop, project)?;
    fs::write(
        applications.join(format!("{}.desktop", project.app_id)),
        desktop,
    )
    .map_err(|error| format!("cannot write Linux desktop entry: {error}"))?;
    fs::copy(
        source.join("application.svg"),
        icons.join(format!("{}.svg", project.app_id)),
    )
    .map_err(|error| format!("cannot copy Linux application icon: {error}"))?;
    let metainfo_source = source.join("application.metainfo.xml");
    let appstream = fs::read_to_string(&metainfo_source).map_err(|error| {
        format!(
            "cannot read Linux AppStream metadata {}: {error}",
            metainfo_source.display()
        )
    })?;
    fs::write(
        metainfo.join(format!("{}.metainfo.xml", project.app_id)),
        rewrite_appstream_identity(&appstream, project)?,
    )
    .map_err(|error| format!("cannot write Linux AppStream metadata: {error}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn rewrite_appstream_identity(source: &str, project: &Project) -> Result<String, String> {
    let mut found_id = false;
    let mut found_name = false;
    let mut found_launchable = false;
    let mut found_binary = false;
    let mut output = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        if trimmed.starts_with("<id>") {
            output.push_str(&format!("{indent}<id>{}</id>\n", xml_text(&project.app_id)));
            found_id = true;
        } else if trimmed.starts_with("<name>") {
            output.push_str(&format!(
                "{indent}<name>{}</name>\n",
                xml_text(&project.title)
            ));
            found_name = true;
        } else if trimmed.starts_with("<launchable type=\"desktop-id\">") {
            output.push_str(&format!(
                "{indent}<launchable type=\"desktop-id\">{}.desktop</launchable>\n",
                xml_text(&project.app_id)
            ));
            found_launchable = true;
        } else if trimmed.starts_with("<binary>") {
            output.push_str(&format!(
                "{indent}<binary>{}</binary>\n",
                xml_text(&project.name)
            ));
            found_binary = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if found_id && found_name && found_launchable && found_binary {
        Ok(output)
    } else {
        Err("Linux AppStream metadata must contain id, name, desktop launchable, and binary elements".into())
    }
}

#[cfg(target_os = "linux")]
fn rewrite_desktop_identity(source: &str, project: &Project) -> Result<String, String> {
    let mut found_name = false;
    let mut found_icon = false;
    let mut output = String::new();
    for line in source.lines() {
        if line.starts_with("Name=") {
            output.push_str(&format!("Name={}\n", project.title));
            found_name = true;
        } else if line.starts_with("Icon=") {
            output.push_str(&format!("Icon={}\n", project.app_id));
            found_icon = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if found_name && found_icon {
        Ok(output)
    } else {
        Err("resources/linux/application.desktop must contain Name= and Icon= entries".into())
    }
}

#[cfg(target_os = "linux")]
fn deploy_linux_qt_runtime(project: &Project, binary: &Path, dist: &Path) -> Result<(), String> {
    let qmake = find_qt_qmake()
        .or_else(|| find_command(["qmake6", "qmake"]))
        .ok_or("cannot deploy Qt: qmake6/qmake was not found in PATH or the gansi Qt root")?;
    let plugin_root = qmake_query(&qmake, "QT_INSTALL_PLUGINS")?;
    let qt_prefix = qmake_query(&qmake, "QT_INSTALL_PREFIX")?;
    let lib_dir = dist.join("lib");
    let plugin_dir = dist.join("plugins");
    fs::create_dir_all(&lib_dir)
        .map_err(|error| format!("cannot create {}: {error}", lib_dir.display()))?;

    let mut scan_queue = VecDeque::from([binary.to_path_buf()]);
    for category in [
        "platforms",
        "platformthemes",
        "imageformats",
        "iconengines",
        "networkinformation",
        "tls",
    ] {
        let source_dir = plugin_root.join(category);
        if !source_dir.is_dir() {
            continue;
        }
        let destination_dir = plugin_dir.join(category);
        fs::create_dir_all(&destination_dir)
            .map_err(|error| format!("cannot create {}: {error}", destination_dir.display()))?;
        for source in shared_objects(&source_dir)? {
            let file_name = source
                .file_name()
                .ok_or_else(|| format!("invalid plugin path {}", source.display()))?;
            if !file_name.to_string_lossy().starts_with("libq") {
                continue;
            }
            let destination = destination_dir.join(file_name);
            copy_qt_object(&source, &destination)
                .map_err(|error| format!("cannot copy Qt plugin {}: {error}", source.display()))?;
            scan_queue.push_back(destination);
        }
    }

    let mut qt_libraries = BTreeMap::new();
    let mut system_libraries = BTreeMap::new();
    while let Some(object) = scan_queue.pop_front() {
        for dependency in dynamic_dependencies(&object)? {
            let Some(file_name) = dependency.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.starts_with("libQt6") {
                system_libraries
                    .entry(file_name.to_string())
                    .or_insert(dependency);
                continue;
            }
            if qt_libraries.contains_key(file_name) {
                continue;
            }
            let destination = lib_dir.join(file_name);
            copy_qt_object(&dependency, &destination).map_err(|error| {
                format!("cannot copy Qt library {}: {error}", dependency.display())
            })?;
            qt_libraries.insert(file_name.to_string(), dependency);
            scan_queue.push_back(destination);
        }
    }

    let launcher = dist.join(&project.name);
    fs::write(
        &launcher,
        format!(
            "#!/bin/sh\nset -eu\nhere=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nexport LD_LIBRARY_PATH=\"$here/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\nexport QT_PLUGIN_PATH=\"$here/plugins${{QT_PLUGIN_PATH:+:$QT_PLUGIN_PATH}}\"\nexec \"$here/libexec/{}\" \"$@\"\n",
            project.name
        ),
    )
    .map_err(|error| format!("cannot write Linux launcher {}: {error}", launcher.display()))?;
    set_executable(&launcher)?;

    let mut inventory = String::from("kind\tbundled_path\tsource_path\n");
    for (name, source) in qt_libraries {
        inventory.push_str(&format!("library\tlib/{name}\t{}\n", source.display()));
    }
    for plugin in shared_objects(&plugin_dir)? {
        let relative = plugin.strip_prefix(dist).unwrap_or(&plugin);
        inventory.push_str(&format!(
            "plugin\t{}\t{}\n",
            relative.display(),
            plugin_root
                .join(relative.strip_prefix("plugins").unwrap_or(relative))
                .display()
        ));
    }
    fs::write(dist.join("QT_RUNTIME.tsv"), inventory)
        .map_err(|error| format!("cannot write Qt runtime inventory: {error}"))?;
    let mut system_inventory = String::from("library\tsource_path\n");
    for (name, source) in system_libraries {
        system_inventory.push_str(&format!("{name}\t{}\n", source.display()));
    }
    fs::write(dist.join("SYSTEM_RUNTIME.tsv"), system_inventory)
        .map_err(|error| format!("cannot write system runtime inventory: {error}"))?;
    write_glibc_requirements(binary, &lib_dir, &plugin_dir, dist)?;
    copy_qt_license_texts(&qt_prefix, dist)?;
    Ok(())
}

/// Locate qmake in the gansi-managed Qt root (recorded by `gansi setup`),
/// falling back to PATH lookups in the caller.
fn find_qt_qmake() -> Option<PathBuf> {
    let config = crate::load_global_config().ok()?;
    let roots = config.qt_roots;
    for root in roots {
        let bin = PathBuf::from(root).join("bin");
        for name in ["qmake6", "qmake"] {
            let candidate = bin.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Copy a Qt library or plugin, preserving symlinks: `fs::copy` would turn
/// soname links into plain files containing the target name, breaking the
/// bundled runtime at launch.
fn copy_qt_object(source: &Path, destination: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(source)?;
        let _ = fs::remove_file(destination);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, destination)?;
        }
        #[cfg(not(unix))]
        {
            fs::copy(&target, destination)?;
        }
        return Ok(());
    }
    fs::copy(source, destination).map(|_| ())
}

#[cfg(target_os = "linux")]
fn write_glibc_requirements(
    binary: &Path,
    lib_dir: &Path,
    plugin_dir: &Path,
    dist: &Path,
) -> Result<(), String> {
    let mut objects = vec![binary.to_path_buf()];
    if lib_dir.is_dir() {
        for entry in fs::read_dir(lib_dir)
            .map_err(|error| format!("cannot read {}: {error}", lib_dir.display()))?
        {
            let path = entry
                .map_err(|error| format!("cannot read library entry: {error}"))?
                .path();
            if path.is_file() {
                objects.push(path);
            }
        }
    }
    objects.extend(shared_objects(plugin_dir)?);
    objects.sort();

    let mut report = String::from("object\tmax_required_glibc\n");
    let mut bundle_max: Option<String> = None;
    for object in objects {
        let output = Command::new("readelf")
            .args(["--version-info", "--wide"])
            .arg(&object)
            .output()
            .map_err(|error| {
                format!(
                    "failed to inspect {} with readelf: {error}",
                    object.display()
                )
            })?;
        if !output.status.success() {
            return Err(format!("readelf failed for {}", object.display()));
        }
        let version = maximum_glibc_version(&String::from_utf8_lossy(&output.stdout));
        if let Some(version) = version.as_ref()
            && bundle_max
                .as_ref()
                .is_none_or(|current| compare_versions(version, current).is_gt())
        {
            bundle_max = Some(version.clone());
        }
        report.push_str(&format!(
            "{}\t{}\n",
            object.strip_prefix(dist).unwrap_or(&object).display(),
            version.as_deref().unwrap_or("none")
        ));
    }
    report.push_str(&format!(
        "<bundle>\t{}\n",
        bundle_max.as_deref().unwrap_or("none")
    ));
    fs::write(dist.join("GLIBC_REQUIREMENTS.tsv"), report)
        .map_err(|error| format!("cannot write GLIBC requirements: {error}"))
}

#[cfg(any(target_os = "linux", test))]
fn maximum_glibc_version(output: &str) -> Option<String> {
    output
        .match_indices("GLIBC_")
        .filter_map(|(index, _)| {
            let suffix = &output[index + "GLIBC_".len()..];
            let version: String = suffix
                .chars()
                .take_while(|character| character.is_ascii_digit() || *character == '.')
                .collect();
            (!version.is_empty()).then_some(version)
        })
        .max_by(|left, right| compare_versions(left, right))
}

#[cfg(any(target_os = "linux", test))]
fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left = left.split('.').map(|part| part.parse::<u64>().unwrap_or(0));
    let mut right = right
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0));
    loop {
        match (left.next(), right.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (left, right) => {
                let ordering = left.unwrap_or(0).cmp(&right.unwrap_or(0));
                if !ordering.is_eq() {
                    return ordering;
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn qmake_query(qmake: &Path, key: &str) -> Result<PathBuf, String> {
    let output = Command::new(qmake)
        .args(["-query", key])
        .output()
        .map_err(|error| format!("failed to query {key} with {}: {error}", qmake.display()))?;
    if !output.status.success() {
        return Err(format!("{} -query {key} failed", qmake.display()));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Err(format!("qmake returned an empty {key}"))
    } else {
        Ok(PathBuf::from(value))
    }
}

#[cfg(target_os = "linux")]
fn find_command<const N: usize>(names: [&str; N]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn shared_objects(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "so") {
                result.push(path);
            }
        }
    }
    result.sort();
    Ok(result)
}

#[cfg(target_os = "linux")]
fn dynamic_dependencies(object: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("ldd")
        .arg(object)
        .output()
        .map_err(|error| format!("failed to inspect {} with ldd: {error}", object.display()))?;
    if !output.status.success() {
        return Err(format!("ldd failed for {}", object.display()));
    }
    parse_ldd_output(&String::from_utf8_lossy(&output.stdout), object)
}

#[cfg(target_os = "linux")]
fn parse_ldd_output(output: &str, object: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dependencies = Vec::new();
    let mut missing = Vec::new();
    for line in output.lines() {
        if let Some((name, target)) = line.split_once("=>") {
            let target = target.trim_start();
            if target.starts_with("not found") {
                missing.push(name.trim().to_string());
                continue;
            }
        }
        let value = line
            .split_once("=>")
            .map_or(line, |(_, path)| path)
            .split_whitespace()
            .next();
        if let Some(value) = value.filter(|value| value.starts_with('/')) {
            dependencies.push(PathBuf::from(value));
        }
    }
    if missing.is_empty() {
        Ok(dependencies)
    } else {
        Err(format!(
            "{} has unresolved shared libraries: {}",
            object.display(),
            missing.join(", ")
        ))
    }
}

#[cfg(target_os = "linux")]
fn copy_qt_license_texts(qt_prefix: &Path, dist: &Path) -> Result<(), String> {
    let destination = dist.join("licenses/qt");
    for source in [qt_prefix.join("LICENSES"), qt_prefix.join("licenses")] {
        if source.is_dir() {
            return copy_directory(&source, &destination);
        }
    }

    let distro_licenses = qt_prefix.join("share/licenses");
    let mut copied = false;
    if distro_licenses.is_dir() {
        for entry in fs::read_dir(&distro_licenses)
            .map_err(|error| format!("cannot read {}: {error}", distro_licenses.display()))?
        {
            let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
            let name = entry.file_name();
            if entry.path().is_dir() && name.to_string_lossy().starts_with("qt6-") {
                copy_directory(&entry.path(), &destination.join(name))?;
                copied = true;
            }
        }
    }
    if !copied {
        fs::write(
            dist.join("QT_LICENSES_NOT_FOUND.txt"),
            "Qt license texts were not found under the detected Qt prefix. Add the applicable Qt and third-party license texts before distribution.\n",
        )
        .map_err(|error| format!("cannot write missing-license warning: {error}"))?;
    };
    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("cannot read {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|error| {
                format!("cannot copy license {}: {error}", entry.path().display())
            })?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("cannot make {} executable: {error}", path.display()))
}

fn write_cargo_license_inventory(project_dir: &Path, dist: &Path) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(project_dir)
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed while collecting licenses: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cannot parse cargo metadata: {error}"))?;
    copy_cargo_license_files(&metadata.packages, dist)?;
    let inventory = render_cargo_license_inventory(metadata.packages);
    fs::write(dist.join("CARGO_LICENSES.tsv"), inventory)
        .map_err(|error| format!("cannot write Cargo license inventory: {error}"))
}

fn copy_cargo_license_files(packages: &[CargoPackage], dist: &Path) -> Result<(), String> {
    let root = dist.join("licenses/cargo");
    for package in packages {
        let package_root = package
            .manifest_path
            .parent()
            .ok_or_else(|| format!("invalid manifest path {}", package.manifest_path.display()))?;
        let mut sources = BTreeMap::<String, PathBuf>::new();
        if let Some(license_file) = package.license_file.as_ref() {
            let path = if license_file.is_absolute() {
                license_file.clone()
            } else {
                package_root.join(license_file)
            };
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                sources.insert(name.to_string(), path);
            }
        }
        for entry in fs::read_dir(package_root)
            .map_err(|error| format!("cannot read {}: {error}", package_root.display()))?
        {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot read license candidate in {}: {error}",
                    package_root.display()
                )
            })?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if path.is_file()
                && (lower.starts_with("license")
                    || lower.starts_with("copying")
                    || lower.starts_with("notice"))
            {
                sources.entry(name.to_string()).or_insert(path);
            }
        }
        if sources.is_empty() {
            let readme = package_root.join("README.md");
            if fs::read_to_string(&readme)
                .is_ok_and(|content| content.contains("SPDX-License-Identifier:"))
            {
                sources.insert("README-SPDX.md".to_string(), readme);
            }
        }
        if sources.is_empty() {
            continue;
        }
        let destination = root.join(format!("{}-{}", package.name, package.version));
        fs::create_dir_all(&destination)
            .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
        for (name, source) in sources {
            fs::copy(&source, destination.join(&name))
                .map_err(|error| format!("cannot copy license {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

fn render_cargo_license_inventory(mut packages: Vec<CargoPackage>) -> String {
    packages.sort_by(|left, right| (&left.name, &left.version).cmp(&(&right.name, &right.version)));
    let mut output = String::from("name\tversion\tlicense\trepository\n");
    for package in packages {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            package.name,
            package.version,
            package.license.as_deref().unwrap_or("UNKNOWN"),
            package.repository.as_deref().unwrap_or("")
        ));
    }
    output
}

#[allow(dead_code)] // used on Windows/macOS only
fn run_deploy_tool(tool: &str, args: &[String], dir: &Path) -> Result<(), String> {
    let output = Command::new(tool)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("failed to run {tool}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{tool} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_validation() {
        assert!(Project::parse("hello-world").is_ok());
        assert!(Project::parse("hello_world2").is_ok());
        assert!(Project::parse("Hello").is_err());
        assert!(Project::parse("hello world").is_err());
        assert!(Project::parse("-hello").is_err());
        assert!(Project::parse("hello-").is_err());
        assert!(Project::parse("").is_err());
    }

    #[test]
    fn derived_names() {
        let project = Project::parse("my-app").unwrap();
        assert_eq!(project.name_snake, "my_app");
        assert_eq!(project.title, "MyApp");
        assert_eq!(project.app_id, "com.example.my-app");
    }

    #[test]
    fn generation_writes_all_files() {
        let mut project = Project::parse("smoke-app").unwrap();
        project.pin_toolchain("6.11.1", "clang", "aarch64");
        let dir = std::env::temp_dir().join(format!("gansi-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_project(&project, &dir).unwrap();
        for file in [
            "Cargo.toml",
            "build.rs",
            "LICENSE-MIT",
            "LICENSE-APACHE",
            ".github/workflows/ci.yml",
            "gansi.toml",
            "RELEASE.md",
            "README.md",
            "THIRD_PARTY_NOTICES.txt",
            "resources/linux/application.desktop",
            "resources/linux/application.metainfo.xml",
            "resources/linux/application.svg",
            "src/main.rs",
            "tests/smoke.rs",
        ] {
            assert!(dir.join(file).exists(), "missing generated file {file}");
        }
        let cargo_toml = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo_toml.contains("name = \"smoke-app\""));
        assert!(cargo_toml.contains("license = \"MIT OR Apache-2.0\""));
        let ci = fs::read_to_string(dir.join(".github/workflows/ci.yml")).unwrap();
        assert!(ci.contains("runs-on: ubuntu-24.04"));
        assert!(ci.contains("cargo add yse --path yse/crates/yse"));
        assert!(ci.contains("YSE_SMOKE: \"1\""));
        let main_rs = fs::read_to_string(dir.join("src/main.rs")).unwrap();
        assert!(main_rs.contains("use yse::*"));
        let gansi_toml = fs::read_to_string(dir.join("gansi.toml")).unwrap();
        assert!(gansi_toml.contains("qt_version = \"6.11.1\""));
        assert!(gansi_toml.contains("compiler_family = \"clang\""));
        assert!(gansi_toml.contains("target_arch = \"aarch64\""));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_parse_pins_the_checkout_path() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let project = Project::parse_local("local-app", &repo.to_string_lossy()).unwrap();
        assert!(project.local_yse.is_some());

        let dir = std::env::temp_dir().join(format!("gansi-local-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_project(&project, &dir).unwrap();
        assert!(dir.join("Cargo.toml").exists());
        assert!(dir.join("build.rs").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_applies_and_validates_deployment_identity() {
        let dir = std::env::temp_dir().join(format!("gansi-identity-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("gansi.toml");
        fs::write(
            &manifest,
            "[project]\nname = \"sample-app\"\n\n[deployment]\nbundle_name = \"Sample Application\"\napp_id = \"org.example.SampleApp\"\n",
        )
        .unwrap();
        let project = Project::from_manifest(manifest.to_str().unwrap()).unwrap();
        assert_eq!(project.title, "Sample Application");
        assert_eq!(project.app_id, "org.example.SampleApp");

        fs::write(
            &manifest,
            "[project]\nname = \"sample-app\"\n\n[deployment]\napp_id = \"not/valid\"\n",
        )
        .unwrap();
        assert!(Project::from_manifest(manifest.to_str().unwrap()).is_err());

        fs::write(
            &manifest,
            "[project]\nname = \"sample-app\"\n\n[deployment]\nbundle_name = \"../escaped\"\n",
        )
        .unwrap();
        assert!(Project::from_manifest(manifest.to_str().unwrap()).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn macos_metadata_uses_deployment_identity_and_escapes_xml() {
        let mut project = Project::parse("sample-app").unwrap();
        project.title = "Sample & <Tools>\"'".to_string();
        project.app_id = "org.example.SampleApp".to_string();

        let plist = macos_info_plist(&project);

        assert!(plist.contains("<string>sample-app</string>"));
        assert!(plist.contains("<string>org.example.SampleApp</string>"));
        assert!(plist.contains("<string>Sample &amp; &lt;Tools&gt;&quot;&apos;</string>"));
        assert!(!plist.contains("dev.yse."));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_identity_is_rendered_from_manifest_project() {
        let mut project = Project::parse("sample-app").unwrap();
        project.title = "Sample Application".to_string();
        project.app_id = "org.example.SampleApp".to_string();
        let desktop = rewrite_desktop_identity(
            "[Desktop Entry]\nName=Old Name\nIcon=com.example.old\nType=Application\n",
            &project,
        )
        .unwrap();
        assert!(desktop.contains("Name=Sample Application\n"));
        assert!(desktop.contains("Icon=org.example.SampleApp\n"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn appstream_identity_is_rendered_from_manifest_project() {
        let mut project = Project::parse("sample-app").unwrap();
        project.title = "Sample & Tools".to_string();
        project.app_id = "org.example.SampleApp".to_string();
        let metadata = rewrite_appstream_identity(template::LINUX_APPSTREAM, &project).unwrap();
        assert!(metadata.contains("<id>org.example.SampleApp</id>"));
        assert!(metadata.contains("<name>Sample &amp; Tools</name>"));
        assert!(metadata.contains(
            "<launchable type=\"desktop-id\">org.example.SampleApp.desktop</launchable>"
        ));
        assert!(metadata.contains("<binary>sample-app</binary>"));
    }

    #[test]
    fn license_inventory_is_sorted_and_marks_unknown_terms() {
        let inventory = render_cargo_license_inventory(vec![
            CargoPackage {
                name: "zeta".into(),
                version: "1.0.0".into(),
                license: None,
                license_file: None,
                repository: None,
                manifest_path: PathBuf::from("/tmp/zeta/Cargo.toml"),
            },
            CargoPackage {
                name: "alpha".into(),
                version: "2.0.0".into(),
                license: Some("MIT".into()),
                license_file: None,
                repository: Some("https://example.invalid/alpha".into()),
                manifest_path: PathBuf::from("/tmp/alpha/Cargo.toml"),
            },
        ]);
        let lines: Vec<_> = inventory.lines().collect();
        assert!(lines[1].starts_with("alpha\t2.0.0\tMIT"));
        assert_eq!(lines[2], "zeta\t1.0.0\tUNKNOWN\t");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ldd_parser_collects_paths_and_rejects_missing_libraries() {
        let paths = parse_ldd_output(
            "libQt6Core.so.6 => /lib64/libQt6Core.so.6 (0x1)\n/lib64/ld-linux-x86-64.so.2 (0x2)\nlinux-vdso.so.1 (0x3)\n",
            Path::new("app"),
        )
        .unwrap();
        assert_eq!(
            paths,
            [
                PathBuf::from("/lib64/libQt6Core.so.6"),
                PathBuf::from("/lib64/ld-linux-x86-64.so.2")
            ]
        );

        let error = parse_ldd_output(
            "libmissing.so.1 => not found\nlibc.so.6 => /lib64/libc.so.6 (0x1)\n",
            Path::new("plugin.so"),
        )
        .expect_err("unresolved libraries must fail deployment");
        assert!(error.contains("plugin.so has unresolved shared libraries: libmissing.so.1"));
    }

    #[test]
    fn glibc_parser_uses_numeric_version_ordering() {
        let output = "Name: GLIBC_2.9  Flags: none\nName: GLIBC_2.34\nName: GLIBC_PRIVATE\nName: GLIBC_2.2.5\n";
        assert_eq!(maximum_glibc_version(output).as_deref(), Some("2.34"));
        assert!(compare_versions("2.10", "2.9").is_gt());
        assert!(compare_versions("2.2.5", "2.2").is_gt());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_archive_ignores_source_timestamps() {
        let project = Project::parse("archive-app").unwrap();
        let root = std::env::temp_dir().join(format!("gansi-archive-{}", std::process::id()));
        let bundle = root.join(&project.name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(bundle.join("share")).unwrap();
        let file = bundle.join("share/data.txt");
        fs::write(&file, "stable content\n").unwrap();

        create_linux_archive(&project, &bundle, &root).unwrap();
        let archive = root.join(format!(
            "{}-linux-{}.tar.gz",
            project.name,
            std::env::consts::ARCH
        ));
        let first = fs::read(&archive).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        fs::write(&file, "stable content\n").unwrap();
        create_linux_archive(&project, &bundle, &root).unwrap();
        let second = fs::read(&archive).unwrap();

        assert_eq!(first, second);
        let _ = fs::remove_dir_all(&root);
    }
}
