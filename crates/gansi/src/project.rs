//! Project model, name handling, and bundle creation.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::template;

pub const MANIFEST_FILE_NAME: &str = "gansi.toml";

pub struct Project {
    pub name: String,
    pub name_snake: String,
    pub title: String,
    pub local_yse: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum BundleProfile {
    Debug,
    Release,
}

impl BundleProfile {
    fn target_dir(&self) -> &'static str {
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
            local_yse: None,
        })
    }

    /// Parse a project name and pin it to a local Yse repository checkout.
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
        project.local_yse = Some(absolute.to_string_lossy().replace('\\', "/"));
        Ok(project)
    }

    /// Read the pinned project configuration from `gansi.toml`.
    pub fn from_manifest(path: &str) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|error| {
            format!("cannot read `{path}` (run `gansi create` first): {error}")
        })?;
        let name = content
            .lines()
            .find_map(|line| line.trim().strip_prefix("name = "))
            .ok_or_else(|| format!("`{path}` is missing `project.name`"))?
            .trim_matches('"')
            .to_string();
        Self::parse(&name)
    }
}

fn title_case(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub fn write_project(project: &Project, target: &Path) -> Result<(), String> {
    let files: Vec<(&str, String)> = vec![
        (
            "Cargo.toml",
            template::render(template::CARGO_TOML, project),
        ),
        ("build.rs", template::render(template::BUILD_RS, project)),
        ("gansi.toml", template::render(template::GANSI_TOML, project)),
        (".gitignore", template::render(template::GITIGNORE, project)),
        (
            "RELEASE.md",
            template::render(template::RELEASE_MD, project),
        ),
        ("README.md", template::render(template::README_MD, project)),
        ("src/main.rs", template::render(template::MAIN_RS, project)),
        (
            "src/bridge.rs",
            template::render(template::BRIDGE_RS, project),
        ),
        ("src/spike.h", template::render(template::SPIKE_H, project)),
        (
            "src/spike.cpp",
            template::render(template::SPIKE_CPP, project),
        ),
        ("tests/smoke.rs", template::render(template::TESTS_SMOKE_RS, project)),
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

/// Generate a project that depends on a local Yse checkout through the
/// `yse` facade (no C++ shim: `yse-ui` provides the Qt surface).
pub fn write_project_local(project: &Project, target: &Path) -> Result<(), String> {
    let files: Vec<(&str, String)> = vec![
        (
            "Cargo.toml",
            template::render_local(template::CARGO_TOML_LOCAL, project),
        ),
        (
            "gansi.toml",
            template::render_local(template::GANSI_TOML, project),
        ),
        ("README.md", template::render_local(template::README_MD, project)),
        (
            ".gitignore",
            template::render_local(template::GITIGNORE, project),
        ),
        (
            "RELEASE.md",
            template::render_local(template::RELEASE_MD, project),
        ),
        (
            "src/main.rs",
            template::render_local(template::MAIN_RS_LOCAL, project),
        ),
        ("tests/smoke.rs", template::render_local(template::TESTS_SMOKE_RS, project)),
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
/// Windows and macOS invoke Qt's deployment tools to make the output portable.
pub fn bundle(project: &Project, profile: BundleProfile) -> Result<(), String> {
    let dist = env::current_dir()
        .map_err(|error| format!("cannot read current directory: {error}"))?
        .join("dist")
        .join(&project.name);
    fs::create_dir_all(&dist)
        .map_err(|error| format!("cannot create {}: {error}", dist.display()))?;

    let binary_name = format!("{}{}", project.name, env::consts::EXE_SUFFIX);
    let binary = env::current_dir()
        .map_err(|error| error.to_string())?
        .join("target")
        .join(profile.target_dir())
        .join(binary_name);
    #[cfg(not(target_os = "macos"))]
    let destination = dist.join(format!("{}{}", project.name, env::consts::EXE_SUFFIX));

    #[cfg(target_os = "macos")]
    let destination = {
        let app = dist.join(format!("{}.app", project.title));
        let macos = app.join("Contents").join("MacOS");
        fs::create_dir_all(&macos)
            .map_err(|error| format!("cannot create {}: {error}", macos.display()))?;
        fs::write(
            app.join("Contents").join("Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleExecutable</key><string>{}</string><key>CFBundleIdentifier</key><string>dev.yse.{}</string><key>CFBundleName</key><string>{}</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>\n",
                project.name, project.name_snake, project.title
            ),
        )
        .map_err(|error| format!("cannot write macOS bundle metadata: {error}"))?;
        macos.join(&project.name)
    };
    fs::copy(&binary, &destination)
        .map_err(|error| format!("cannot copy binary {}: {error}", binary.display()))?;

    let qt_platform = env::var("QT_QPA_PLATFORM").unwrap_or_default();
    if qt_platform == "offscreen" {
        // Headless CI bundles skip platform deployment tooling.
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    run_deploy_tool(
        "windeployqt",
        &[destination.to_string_lossy().to_string()],
        &dist,
    )?;
    #[cfg(target_os = "macos")]
    run_deploy_tool(
        "macdeployqt",
        &[dist
            .join(format!("{}.app", project.title))
            .to_string_lossy()
            .to_string()],
        &dist,
    )?;

    Ok(())
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
    }

    #[test]
    fn generation_writes_all_files() {
        let project = Project::parse("smoke-app").unwrap();
        let dir = std::env::temp_dir().join(format!("gansi-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_project(&project, &dir).unwrap();
        for file in [
            "Cargo.toml",
            "build.rs",
            "gansi.toml",
            "RELEASE.md",
            "README.md",
            "src/main.rs",
            "src/bridge.rs",
            "src/spike.h",
            "src/spike.cpp",
            "tests/smoke.rs",
        ] {
            assert!(dir.join(file).exists(), "missing generated file {file}");
        }
        let cargo_toml = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo_toml.contains("name = \"smoke-app\""));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_generation_uses_the_facade() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let project = Project::parse_local("local-app", &repo.to_string_lossy()).unwrap();
        assert!(project.local_yse.is_some());

        let dir = std::env::temp_dir().join(format!("gansi-local-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        write_project_local(&project, &dir).unwrap();
        let cargo_toml = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo_toml.contains("yse = { path = \""));
        assert!(cargo_toml.contains("/crates/yse\""));
        assert!(!dir.join("build.rs").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
