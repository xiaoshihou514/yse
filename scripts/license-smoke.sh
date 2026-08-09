#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
metadata="$(mktemp "${TMPDIR:-/tmp}/yse-license-smoke.XXXXXX")"
trap 'rm -f "$metadata"' EXIT

command -v jq >/dev/null || {
    echo "jq is required for the license smoke gate" >&2
    exit 1
}

cargo metadata --manifest-path "$repo_root/Cargo.toml" --locked --offline \
    --format-version 1 >"$metadata"

missing="$(
    jq -r '.packages[] | select((.license == null or .license == "") and .license_file == null) | "\(.name) \(.version)"' \
        "$metadata"
)"
if [[ -n "$missing" ]]; then
    printf 'Packages without license metadata or a license file:\n%s\n' "$missing" >&2
    exit 1
fi

unexpected_workspace="$(
    jq -r '
        .workspace_members as $members
        | .packages[]
        | select(.id as $id | $members | index($id))
        | select(.license != "MIT OR Apache-2.0")
        | "\(.name) \(.version): \(.license // "UNKNOWN")"
    ' "$metadata"
)"
if [[ -n "$unexpected_workspace" ]]; then
    printf 'Workspace packages with unexpected license terms:\n%s\n' \
        "$unexpected_workspace" >&2
    exit 1
fi

unexpected_cxx="$(
    jq -r '
        .packages[]
        | select(.name | startswith("cxx"))
        | select(.license != "MIT OR Apache-2.0")
        | "\(.name) \(.version): \(.license // "UNKNOWN")"
    ' "$metadata"
)"
if [[ -n "$unexpected_cxx" ]]; then
    printf 'Locked CXX/CXX-Qt packages with unexpected license terms:\n%s\n' \
        "$unexpected_cxx" >&2
    exit 1
fi

echo "Locked dependency graph has complete license declarations"
echo "Workspace and CXX/CXX-Qt packages declare MIT OR Apache-2.0"
