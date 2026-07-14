tauri := "npx @tauri-apps/cli@^2"

# 检查 core 编译
check-core:
    cargo check -p yse-core
    cargo test -p yse-core

check-frontend:
    cd frontend && npx --package vue-tsc vue-tsc --noEmit

# Rust clippy 检查
clippy:
    cd desktop && {{ tauri }} icon ../icon.png 2>/dev/null; true
    cd mobile && {{ tauri }} icon ../icon.png 2>/dev/null; true
    cp desktop/icons/32x32.png frontend/public/icon.png 2>/dev/null; true
    cargo clippy -- -D warnings

# 前端 + Rust 全部检查
check: check-frontend check-core clippy

frontend:
    cd frontend && npm run build

# Tauri 开发模式 (启动 vite + tauri)
dev: plugin-all
    cd desktop && cargo tauri dev

# 构建 AppImage
appimage:
    cd desktop && {{ tauri }} icon ../icon.png && cp icons/32x32.png ../frontend/public/icon.png
    cd desktop && {{ tauri }} build --bundles appimage

# 完整构建 Android APK (前端 + NDK + 签名)
android:
    bash scripts/android-build.sh

# 编译 echo-bot 插件
plugin-echo:
    cd plugins/echo-bot && cargo build --release

# 编译 opencode-bot 插件
plugin-opencode:
    cd plugins/opencode-bot && npm install && npm run build

# 编译 file-tree 插件
plugin-file-tree:
    cd plugins/file-tree && cargo build --release

# 编译 project-manager 插件
plugin-pm:
    cd plugins/project-manager && cargo build --release

# 编译所有插件
plugin-all: plugin-echo plugin-file-tree plugin-pm plugin-opencode

# 格式化（frontend prettier + rustfmt）
format:
    cd frontend && npm run format
    cargo fmt --all

# 清理构建产物
clean:
    cargo clean
    cd frontend && rm -rf dist node_modules/.vite
