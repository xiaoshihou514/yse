#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_log="$(mktemp "${TMPDIR:-/tmp}/yse-memory-smoke.XXXXXX")"
trap 'rm -f "$build_log"' EXIT

command -v valgrind >/dev/null || {
    echo "valgrind is required for the memory smoke gate" >&2
    exit 1
}
command -v jq >/dev/null || {
    echo "jq is required for the memory smoke gate" >&2
    exit 1
}

export CCACHE_DISABLE=1
export CARGO_NET_OFFLINE=true

cargo test --manifest-path "$repo_root/Cargo.toml" -p yse-ui --test soak \
    --no-run --message-format=json >"$build_log"
test_binary="$(
    jq -r 'select(.reason == "compiler-artifact" and .target.name == "soak" and .executable != null) | .executable' \
        "$build_log" | tail -n 1
)"
test -x "$test_binary"

QT_QPA_PLATFORM=offscreen valgrind \
    --quiet \
    --tool=memcheck \
    --leak-check=full \
    --show-leak-kinds=definite \
    --errors-for-leak-kinds=definite \
    --error-exitcode=97 \
    --suppressions="$repo_root/scripts/valgrind-qt.supp" \
    "$test_binary" --nocapture

echo "Yse UI lifecycle passed the Valgrind memory smoke gate"
