#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state_dir="$(mktemp -d "${TMPDIR:-/tmp}/yse-examples-smoke.XXXXXX")"
trap 'rm -rf "$state_dir"' EXIT

export CCACHE_DISABLE=1
export CARGO_INCREMENTAL=0
export CARGO_NET_OFFLINE=true
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}"
export YSE_SMOKE=1
export XDG_CACHE_HOME="$state_dir/cache"
export XDG_CONFIG_HOME="$state_dir/config"
export XDG_DATA_HOME="$state_dir/data"
mkdir -p "$XDG_CACHE_HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME"
smoke_timeout="${YSE_SMOKE_TIMEOUT_SECONDS:-90}"

run_ui_example() {
    local name="$1"
    local expected="${2:-}"
    echo "==> Smoke testing yse-ui example: $name"
    local output
    if ! output="$(timeout "$smoke_timeout" cargo run --manifest-path "$repo_root/Cargo.toml" -p yse-ui --example "$name" 2>&1)"; then
        printf '%s\n' "$output" >&2
        return 1
    fi
    printf '%s\n' "$output"
    if [[ -n "$expected" ]]; then
        grep -Fq "$expected" <<<"$output"
    fi
}

run_ui_example settings '[settings]'
run_ui_example controls '[controls]'
run_ui_example data_browser \
    '[data-browser] status="error: simulated failure" rows=1 filter="grace" first="Grace Hopper"'
run_ui_example kcalc
run_ui_example kalarm
run_ui_example filelight

echo "==> Smoke testing workspace application: media-converter"
export YSE_SMOKE_DIR="$state_dir/media-converter"
if ! media_converter_output="$(timeout "$smoke_timeout" cargo run --manifest-path "$repo_root/Cargo.toml" -p yse-media-converter 2>&1)"; then
    printf '%s\n' "$media_converter_output" >&2
    exit 1
fi
printf '%s\n' "$media_converter_output"
grep -Fq '[media_converter] output=' <<<"$media_converter_output"
grep -Fq '[media_converter] Conversion complete.' <<<"$media_converter_output"
test -s "$YSE_SMOKE_DIR/yse-media-smoke.flac"
unset YSE_SMOKE_DIR

echo "==> Smoke testing workspace application: taskmgr"
if ! taskmgr_output="$(timeout "$smoke_timeout" cargo run --manifest-path "$repo_root/Cargo.toml" -p yse-taskmgr 2>&1)"; then
    printf '%s\n' "$taskmgr_output" >&2
    exit 1
fi
printf '%s\n' "$taskmgr_output"
grep -Fq '[taskmgr] processes=' <<<"$taskmgr_output"
grep -Fq '[diag] pages=' <<<"$taskmgr_output"

echo "All Linux examples passed their $QT_QPA_PLATFORM smoke runs"
