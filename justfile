# ── 盐水鹅 开发命令 ────────────────────────────────────────────────────
# 使用: just <命令>
# 安装: cargo install just

tauri := "npx @tauri-apps/cli@^2"

# ── 核心库 ──────────────────────────────────────────────────────────────

# 检查 core 编译
check-core:
    cargo check -p yse-core

# 运行 core 测试
test-core:
    cargo test -p yse-core

# ── 前端 ────────────────────────────────────────────────────────────────

# 构建前端
fe-build:
    cd frontend && npm run build

# 前端 TypeScript 类型检查
fe-typecheck:
    cd frontend && npx --package vue-tsc vue-tsc --noEmit

# ── Tauri 桌面端 ────────────────────────────────────────────────────────

# 前端开发服务器 (Vite)
fe-dev:
    cd frontend && npm run dev

# Tauri 开发模式 (启动 vite + tauri)
dev:
    cd desktop && cargo tauri dev

# 构建 AppImage
build-appimage:
    cd desktop && {{ tauri }} icon ../icon.png && cp icons/32x32.png ../frontend/public/icon.png
    cd desktop && {{ tauri }} build --bundles appimage

# ── Tauri 移动端 ────────────────────────────────────────────────────────

# 初始化 Android 项目 (仅首次)
android-init:
    cd mobile && {{ tauri }} android init

# 完整构建 Android APK (前端 + NDK + 签名)
android-build:
    bash scripts/android-build.sh

# ── 插件 ────────────────────────────────────────────────────────────────

# 编译 echo-bot 插件
plugin-echo:
    cd plugins/echo-bot && cargo build

# 编译 opencode-bot 插件
plugin-opencode:
    cd plugins/opencode-bot && npm install && npm run build

# 编译 file-tree 插件
plugin-file-tree:
    cd plugins/file-tree && cargo build

# ── Lint / 格式化 ───────────────────────────────────────────────────────

# 格式化（frontend prettier + rustfmt）
format:
    cd frontend && npm run format
    cargo fmt --all

# Rust clippy 检查
clippy:
    cd desktop && {{ tauri }} icon ../icon.png 2>/dev/null; true
    cd mobile && {{ tauri }} icon ../icon.png 2>/dev/null; true
    cp desktop/icons/32x32.png frontend/public/icon.png 2>/dev/null; true
    cargo clippy -- -D warnings

# 前端 + Rust 全部检查
check-all: fe-typecheck check-core clippy

# ── 其他 ────────────────────────────────────────────────────────────────

# 清理构建产物
clean:
    cargo clean
    cd frontend && rm -rf dist node_modules/.vite
