#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

NEED_BUILD=0
# If dist/ doesn't exist, must build
if [ ! -f dist/index.js ]; then
    NEED_BUILD=1
else
    # Check if any src/ file is newer than dist/index.js
    DIST_MTIME=$(stat -c %Y dist/index.js 2>/dev/null || stat -f %m dist/index.js 2>/dev/null || echo 0)
    for f in src/*.ts; do
        SRC_MTIME=$(stat -c %Y "$f" 2>/dev/null || stat -f %m "$f" 2>/dev/null || echo 0)
        if [ "$SRC_MTIME" -gt "$DIST_MTIME" ] 2>/dev/null; then
            NEED_BUILD=1
            break
        fi
    done
fi

if [ "$NEED_BUILD" = 1 ]; then
    echo "[opencode-bot] building..."
    npm run build
fi

exec node dist/index.js
