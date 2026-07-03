# ── YSE (盐水鹅) 开发命令 ──────────────────────────────────────────────
# 使用: just <命令>
# 安装: cargo install just

# ── 核心库 ──────────────────────────────────────────────────────────────

# 检查 core 编译
check-core:
    cargo check -p yse-core

# 运行 core 测试
test-core:
    cargo test -p yse-core

# 全 workspace 检查
check:
    cargo check

# 全 workspace 测试
test:
    cargo test -p yse-core

# ── 前端 ────────────────────────────────────────────────────────────────

# 启动 Vite 开发服务器 (localhost:1420)
fe-dev:
    cd frontend && npm run dev

# 构建前端
fe-build:
    cd frontend && npm run build

# 前端 TypeScript 类型检查
fe-typecheck:
    cd frontend && npx --package vue-tsc vue-tsc --noEmit

# 前端依赖安装
fe-install:
    cd frontend && npm install

# ── Tauri 桌面端 ────────────────────────────────────────────────────────

# Tauri 开发模式 (启动 vite + tauri)
dev:
    cargo tauri dev

# 构建 AppImage
build-appimage:
    cd desktop && npx @tauri-apps/cli@^2 build --bundles appimage

# 构建 deb
build-deb:
    cd desktop && npx @tauri-apps/cli@^2 build --bundles deb

# ── Tauri 移动端 ────────────────────────────────────────────────────────

# 初始化 Android 项目 (仅首次)
android-init:
    cd mobile && npx @tauri-apps/cli@^2 android init

# 构建 Android APK
android-build:
    cd mobile && npx @tauri-apps/cli@^2 android build --apk

# ── 插件 ────────────────────────────────────────────────────────────────

# 编译 echo-bot 插件
plugin-echo:
    cd plugins/echo-bot && cargo build

# ── Lint / 格式化 ───────────────────────────────────────────────────────

# 格式化 Rust 代码
fmt:
    cargo fmt

# Rust clippy 检查
clippy:
    cargo clippy -- -D warnings

# 前端 + Rust 全部检查
check-all: fe-typecheck check

# ── Git ─────────────────────────────────────────────────────────────────

# 查看未提交改动
diff:
    git diff

# 查看已暂存改动
diff-staged:
    git diff --cached --stat

# 最近日志
log:
    git log --oneline -10

# ── 其他 ────────────────────────────────────────────────────────────────

# 清理构建产物
clean:
    cargo clean
    cd frontend && rm -rf dist node_modules/.vite

# 帮助
help:
    @just --list
