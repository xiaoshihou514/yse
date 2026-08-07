//! File templates for `gansi create`. Tokens `{{name}}`, `{{name_snake}}`,
//! `{{title}}`, and `{{gansi_version}}` are substituted by [`render`].

use crate::project::Project;

pub fn render(template: &str, project: &Project) -> String {
    template
        .replace("{{name}}", &project.name)
        .replace("{{name_snake}}", &project.name_snake)
        .replace("{{title}}", &project.title)
        .replace("{{gansi_version}}", env!("CARGO_PKG_VERSION"))
}

pub fn render_local(template: &str, project: &Project) -> String {
    let rendered = render(template, project);
    match &project.local_yse {
        Some(path) => rendered.replace("{{yse_path}}", path),
        None => rendered,
    }
}

/// Facade-based template: depends on a local Yse checkout and needs no C++
/// shim of its own.
pub const CARGO_TOML_LOCAL: &str = include_str!("../templates/cargo_toml_local.template");
pub const MAIN_RS_LOCAL: &str = include_str!("../templates/main_rs_local.template");
pub const CARGO_TOML: &str = include_str!("../templates/cargo_toml.template");
pub const BUILD_RS: &str = include_str!("../templates/build_rs.template");
pub const GANSI_TOML: &str = include_str!("../templates/gansi_toml.template");
pub const GITIGNORE: &str = include_str!("../templates/gitignore.template");
pub const MAIN_RS: &str = include_str!("../templates/main_rs.template");
pub const BRIDGE_RS: &str = include_str!("../templates/bridge_rs.template");
pub const SPIKE_H: &str = include_str!("../templates/spike_h.template");
pub const SPIKE_CPP: &str = include_str!("../templates/spike_cpp.template");
pub const RELEASE_MD: &str = include_str!("../templates/release_md.template");
pub const TESTS_SMOKE_RS: &str = include_str!("../templates/tests_smoke_rs.template");
pub const README_MD: &str = include_str!("../templates/readme_md.template");
