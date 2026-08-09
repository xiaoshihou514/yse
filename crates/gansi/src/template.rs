//! File templates for `gansi create`. Tokens `{{name}}`, `{{name_snake}}`,
//! `{{title}}`, toolchain pins, and `{{gansi_version}}` are substituted by
//! [`render`].

use crate::project::Project;

pub fn render(template: &str, project: &Project) -> String {
    template
        .replace("{{name}}", &project.name)
        .replace("{{name_snake}}", &project.name_snake)
        .replace("{{title}}", &project.title)
        .replace("{{app_id}}", &project.app_id)
        .replace("{{qt_version}}", &project.qt_version)
        .replace("{{compiler_family}}", &project.compiler_family)
        .replace("{{target_arch}}", &project.target_arch)
        .replace("{{gansi_version}}", env!("CARGO_PKG_VERSION"))
}

/// Facade-based template: the project depends on the `yse` facade and needs
/// no C++ shim of its own. Its minimal build script only asks CXX-Qt to retain
/// dependency initializers in the final executable.
pub const CARGO_TOML: &str = include_str!("../templates/cargo_toml.template");
pub const BUILD_RS: &str = include_str!("../templates/build_rs.template");
pub const GANSI_TOML: &str = include_str!("../templates/gansi_toml.template");
pub const GITIGNORE: &str = include_str!("../templates/gitignore.template");
pub const MAIN_RS: &str = include_str!("../templates/main_rs.template");
pub const RELEASE_MD: &str = include_str!("../templates/release_md.template");
pub const TESTS_SMOKE_RS: &str = include_str!("../templates/tests_smoke_rs.template");
pub const README_MD: &str = include_str!("../templates/readme_md.template");
pub const THIRD_PARTY_NOTICES: &str = include_str!("../templates/third_party_notices_txt.template");
pub const LINUX_DESKTOP: &str = include_str!("../templates/linux_desktop.template");
pub const LINUX_APPSTREAM: &str = include_str!("../templates/linux_appstream.template");
pub const APPLICATION_SVG: &str = include_str!("../templates/application_svg.template");
pub const GITHUB_CI_YML: &str = include_str!("../templates/github_ci_yml.template");
pub const LICENSE_MIT: &str = include_str!("../LICENSE-MIT");
pub const LICENSE_APACHE: &str = include_str!("../LICENSE-APACHE");
