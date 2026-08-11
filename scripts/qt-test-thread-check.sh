#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failed=false

for file in "$repo_root"/crates/yse-ui/tests/*.rs; do
    test_count="$(rg -c '^#\[test\]' "$file" || true)"
    app_count="$(rg -c 'Application::init' "$file" || true)"
    if (( test_count > 1 && app_count > 0 )); then
        echo "Qt test binary exposes multiple harness tests: ${file#"$repo_root/"}" >&2
        failed=true
    fi
done

if [[ "$failed" == true ]]; then
    echo "Consolidate Qt cases into one #[test] so QApplication stays on one thread." >&2
    exit 1
fi

echo "Qt integration test binaries keep QApplication on one harness thread"
