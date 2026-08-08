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

/// Facade-based template: the project depends on the `yse` facade and needs
/// no C++ shim of its own. The dependency itself is added by `gansi create`
/// via `cargo add yse`, not baked into the manifest template.
pub const CARGO_TOML: &str = include_str!("../templates/cargo_toml.template");
pub const GANSI_TOML: &str = include_str!("../templates/gansi_toml.template");
pub const GITIGNORE: &str = include_str!("../templates/gitignore.template");
pub const MAIN_RS: &str = include_str!("../templates/main_rs.template");
pub const RELEASE_MD: &str = include_str!("../templates/release_md.template");
pub const TESTS_SMOKE_RS: &str = include_str!("../templates/tests_smoke_rs.template");
pub const README_MD: &str = include_str!("../templates/readme_md.template");
