#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

qml_pattern='QQml|QQuick|QtQml|QtQuick|import[[:space:]]+Qt'
mapfile -d '' implementation_files < <(
    find crates/yse-ui/src -type f \
        \( -name '*.rs' -o -name '*.cpp' -o -name '*.h' \) -print0
)
generated_files=(
    crates/gansi/templates/cargo_toml.template
    crates/gansi/templates/build_rs.template
    crates/gansi/templates/main_rs.template
)

if grep -Eni "$qml_pattern" "${implementation_files[@]}" "${generated_files[@]}"; then
    echo "QML or Qt Quick entered the Widgets-only implementation or generated application API" >&2
    exit 1
fi

if grep -Eni '(^|[^[:alnum:]_-])(cxx|cxx-qt|qt)([^[:alnum:]_-]|$)' crates/yse-model/Cargo.toml; then
    echo "yse-model must remain independent of Qt and the CXX bridge" >&2
    exit 1
fi

grep -Fqx '#![forbid(unsafe_code)]' crates/yse-model/src/lib.rs
grep -Eq '^yse-model[[:space:]]*=' crates/yse-ui/Cargo.toml
grep -Eq '^yse-model[[:space:]]*=' crates/yse/Cargo.toml
grep -Eq '^yse-ui[[:space:]]*=' crates/yse/Cargo.toml

echo "Widgets-only API and workspace dependency direction are intact"
