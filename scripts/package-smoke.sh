#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

export CCACHE_DISABLE=1
export CARGO_NET_OFFLINE=true

package_version() {
    cargo pkgid -p "$1" | sed 's/.*#//'
}

model_version="$(package_version yse-model)"
ui_version="$(package_version yse-ui)"
yse_version="$(package_version yse)"
gansi_version="$(package_version gansi)"
package_dir="$repo_root/target/package"
model_source="$package_dir/yse-model-$model_version"
ui_source="$package_dir/yse-ui-$ui_version"

echo "==> Packaging and verifying yse-model"
cargo package -p yse-model --allow-dirty --offline

echo "==> Packaging and verifying gansi"
cmp LICENSE-MIT crates/gansi/LICENSE-MIT
cmp LICENSE-APACHE crates/gansi/LICENSE-APACHE
cargo package -p gansi --allow-dirty --offline

echo "==> Packaging and verifying yse-ui against packaged yse-model"
cargo package -p yse-ui --allow-dirty --offline \
    --config "patch.crates-io.yse-model.path=\"$model_source\""

echo "==> Packaging and verifying yse facade against packaged dependencies"
cargo package -p yse --allow-dirty --offline \
    --config "patch.crates-io.yse-model.path=\"$model_source\"" \
    --config "patch.crates-io.yse-ui.path=\"$ui_source\""

assert_member() {
    local archive="$1"
    local member="$2"
    tar -tzf "$archive" "$member" >/dev/null
}

assert_member "$package_dir/yse-model-$model_version.crate" "yse-model-$model_version/src/lib.rs"
assert_member "$package_dir/yse-ui-$ui_version.crate" "yse-ui-$ui_version/build.rs"
assert_member "$package_dir/yse-ui-$ui_version.crate" "yse-ui-$ui_version/src/widgets.cpp"
assert_member "$package_dir/yse-ui-$ui_version.crate" "yse-ui-$ui_version/src/widgets.h"
assert_member "$package_dir/yse-$yse_version.crate" "yse-$yse_version/build.rs"
assert_member "$package_dir/gansi-$gansi_version.crate" "gansi-$gansi_version/LICENSE-MIT"
assert_member "$package_dir/gansi-$gansi_version.crate" "gansi-$gansi_version/LICENSE-APACHE"
assert_member "$package_dir/gansi-$gansi_version.crate" \
    "gansi-$gansi_version/templates/linux_appstream.template"
assert_member "$package_dir/gansi-$gansi_version.crate" \
    "gansi-$gansi_version/templates/github_ci_yml.template"

echo "All publishable crates passed standalone package verification"
