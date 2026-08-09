#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/yse-linux-smoke.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT

export CCACHE_DISABLE=1
export QT_QPA_PLATFORM=offscreen
export CARGO_TARGET_DIR="$repo_root/target"
export CARGO_INCREMENTAL=0
export CARGO_NET_OFFLINE=true
export GANSI_HOME="$work_dir/gansi-home"
bundle_profile="${YSE_SMOKE_BUNDLE_PROFILE:-release}"
case "$bundle_profile" in
    release) bundle_args=() ;;
    debug) bundle_args=(--debug) ;;
    *)
        echo "YSE_SMOKE_BUNDLE_PROFILE must be release or debug" >&2
        exit 2
        ;;
esac
qmake_bin="$(command -v qmake6 || command -v qmake)"
qt_version="$($qmake_bin -query QT_VERSION)"
qt_root="$($qmake_bin -query QT_INSTALL_PREFIX)"

echo "==> Building gansi"
cargo build --manifest-path "$repo_root/Cargo.toml" -p gansi
gansi="$CARGO_TARGET_DIR/debug/gansi"

echo "==> Checking the Linux toolchain"
"$gansi" config set qt_roots "$qt_root"
"$gansi" setup "$qt_version"
test "$("$gansi" config get default_qt_version)" = "$qt_version"
test "$("$gansi" config get qt_roots)" = "$qt_root"
"$gansi" doctor --verbose

echo "==> Initializing an existing source directory"
mkdir "$work_dir/init-app"
cd "$work_dir/init-app"
printf 'keep-me\n' > .gitignore
"$gansi" init
test -s gansi.toml
grep -Fqx 'keep-me' .gitignore
manifest_digest="$(sha256sum gansi.toml)"
gitignore_digest="$(sha256sum .gitignore)"
"$gansi" init
test "$(sha256sum gansi.toml)" = "$manifest_digest"
test "$(sha256sum .gitignore)" = "$gitignore_digest"

echo "==> Creating a facade-based application"
cd "$work_dir"
if "$gansi" create unpublished-app 2>create-error.log; then
    echo "unpublished create unexpectedly succeeded without --local" >&2
    exit 1
fi
grep -q 'Yse is not published yet' create-error.log
test ! -e unpublished-app
"$gansi" create --local "$repo_root" smoke-app
cd smoke-app
test -f .github/workflows/ci.yml
grep -q 'cargo add yse --path yse/crates/yse' .github/workflows/ci.yml
grep -q 'YSE_SMOKE: "1"' .github/workflows/ci.yml
grep -Fqx "qt_version = \"$qt_version\"" gansi.toml
grep -Fqx 'rust-version = "1.89"' Cargo.toml
grep -Fqx 'Copyright (c) 2026 xiaoshihou' LICENSE-MIT
grep -Fq 'Apache License' LICENSE-APACHE
grep -q '^compiler_family = "gcc"' gansi.toml
grep -q '^target_arch = "x86_64"$' gansi.toml
desktop-file-validate resources/linux/application.desktop
appstreamcli validate --no-net --override=url-homepage-missing=pedantic resources/linux/application.metainfo.xml
sed -i 's/^bundle_name = .*/bundle_name = "Smoke Application"/' gansi.toml
sed -i 's/^app_id = .*/app_id = "org.example.SmokeApp"/' gansi.toml

echo "==> Testing the generated application"
"$gansi" test

echo "==> Checking generated-project developer workflows"
"$gansi" format --check
"$gansi" analyze
"$gansi" add serde -- --optional --offline
grep -Eq '^serde = \{ version = "[^"]+", optional = true \}$' Cargo.toml
"$gansi" upgrade -- --offline

echo "==> Running the generated application offscreen"
run_output="$(YSE_SMOKE=1 "$gansi" run 2>&1)"
printf '%s\n' "$run_output"
grep -q '^count=1$' <<<"$run_output"

echo "==> Building the $bundle_profile bundle"
mkdir -p dist/smoke-app
touch dist/smoke-app/stale-file
"$gansi" build "${bundle_args[@]}"
test ! -e dist/smoke-app/stale-file
test -x dist/smoke-app/smoke-app
test -s dist/smoke-app-linux-x86_64.tar.gz
test -s dist/SHA256SUMS
(cd dist && sha256sum --check SHA256SUMS)
first_archive_checksum="$(sha256sum dist/smoke-app-linux-x86_64.tar.gz | cut -d ' ' -f 1)"
echo "==> Rebuilding to verify deterministic archive output"
"$gansi" build "${bundle_args[@]}"
second_archive_checksum="$(sha256sum dist/smoke-app-linux-x86_64.tar.gz | cut -d ' ' -f 1)"
test "$first_archive_checksum" = "$second_archive_checksum"
test -x dist/smoke-app/libexec/smoke-app
test -s dist/smoke-app/THIRD_PARTY_NOTICES.txt
grep -Fqx 'Copyright (c) 2026 xiaoshihou' dist/smoke-app/licenses/rust/LICENSE-MIT
grep -Fq 'Apache License' dist/smoke-app/licenses/rust/LICENSE-APACHE
grep -Fq 'Klarälvdalens Datakonsult AB' dist/smoke-app/licenses/cargo/cxx-qt-0.9.1/README-SPDX.md
grep -Fq 'SPDX-License-Identifier: MIT OR Apache-2.0' dist/smoke-app/licenses/cargo/cxx-qt-0.9.1/README-SPDX.md
test -s dist/smoke-app/licenses/cargo/cxx-1.0.198/LICENSE-MIT
test -s dist/smoke-app/licenses/cargo/cxx-1.0.198/LICENSE-APACHE
grep -q $'^cxx-qt\t0.9.1\tMIT OR Apache-2.0\t' dist/smoke-app/CARGO_LICENSES.tsv
grep -q $'^smoke-app\t0.1.0\tMIT OR Apache-2.0\t' dist/smoke-app/CARGO_LICENSES.tsv
if grep -q $'\tUNKNOWN\t' dist/smoke-app/CARGO_LICENSES.tsv; then
    echo "bundle contains a Cargo package without declared license terms" >&2
    exit 1
fi
grep -q $'^library\tlib/libQt6Core.so.6\t' dist/smoke-app/QT_RUNTIME.tsv
grep -q $'^libc.so.6\t/' dist/smoke-app/SYSTEM_RUNTIME.tsv
grep -Eq $'^libexec/smoke-app\t[0-9]+(\.[0-9]+)+$' dist/smoke-app/GLIBC_REQUIREMENTS.tsv
grep -Eq $'^<bundle>\t[0-9]+(\.[0-9]+)+$' dist/smoke-app/GLIBC_REQUIREMENTS.tsv
echo "Bundle GLIBC requirement: $(awk -F '\t' '$1 == "<bundle>" { print $2 }' dist/smoke-app/GLIBC_REQUIREMENTS.tsv)"
test -f dist/smoke-app/plugins/platforms/libqoffscreen.so
test -f dist/smoke-app/licenses/qt/qt6-qtbase/LGPL-3.0-only.txt
desktop_file=dist/smoke-app/share/applications/org.example.SmokeApp.desktop
desktop-file-validate "$desktop_file"
grep -q '^Name=Smoke Application$' "$desktop_file"
grep -q '^Icon=org.example.SmokeApp$' "$desktop_file"
test -f dist/smoke-app/share/icons/hicolor/scalable/apps/org.example.SmokeApp.svg
metainfo_file=dist/smoke-app/share/metainfo/org.example.SmokeApp.metainfo.xml
appstreamcli validate --no-net --override=url-homepage-missing=pedantic "$metainfo_file"
grep -q '<id>org.example.SmokeApp</id>' "$metainfo_file"
grep -q '<name>Smoke Application</name>' "$metainfo_file"

echo "==> Running the bundled Qt runtime"
mkdir "$work_dir/extracted"
tar -xzf dist/smoke-app-linux-x86_64.tar.gz -C "$work_dir/extracted"
extracted_bundle="$work_dir/extracted/smoke-app"
bundle_output="$(LD_DEBUG=libs YSE_SMOKE=1 "$extracted_bundle/smoke-app" 2>&1)"
grep -q '^count=1$' <<<"$bundle_output"
grep -Fq "$extracted_bundle/lib/libQt6Core.so.6" <<<"$bundle_output"
test -s "$extracted_bundle/GLIBC_REQUIREMENTS.tsv"

echo "==> Cleaning isolated build artifacts"
clean_target="$work_dir/clean-target"
mkdir -p "$clean_target/nested" dist/clean-marker
touch "$clean_target/nested/object.o" dist/clean-marker/artifact
CARGO_TARGET_DIR="$clean_target" "$gansi" clean
test ! -e "$clean_target"
test ! -e dist

echo "Linux smoke test passed: create -> test -> run -> bundle"
