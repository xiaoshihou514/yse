# ── YSE (盐水鹅) 开发命令 ──────────────────────────────────────────────
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
    cd desktop && {{ tauri }} build --bundles appimage

# 构建 deb
build-deb:
    cd desktop && {{ tauri }} build --bundles deb

# ── Tauri 移动端 ────────────────────────────────────────────────────────

# 初始化 Android 项目 (仅首次)
android-init:
    cd mobile && {{ tauri }} android init

# 完整构建 Android APK (前端 + NDK + 签名)
android-build:
    #!/usr/bin/env bash
    set -euo pipefail
    cd frontend && npm run build
    mkdir -p ~/.android
    KEYSTORE=~/.android/debug.keystore
    if [ ! -f "$KEYSTORE" ]; then
        keytool -genkey -v -keystore "$KEYSTORE" \
            -alias androiddebugkey -storepass android -keypass android \
            -keyalg RSA -keysize 2048 -validity 10000 \
            -dname "CN=Android Debug,O=Android,C=US"
    fi
    cd mobile && {{ tauri }} android init || true
    cd mobile && npm install
    NDK_DIR=$(ls -1d "${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/ndk/"*/ 2>/dev/null | head -1)
    if [ -n "$NDK_DIR" ]; then
        export TARGET_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
        export CARGO_TARGET_AARCH64_LINUX_ANDROID_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
        export PATH="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
    fi
    cd mobile && {{ tauri }} android build --apk
    BUILD_TOOLS="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/build-tools"
    TOOLS_DIR=$(ls -1d "$BUILD_TOOLS"/*/ 2>/dev/null | head -1)
    if [ -n "$TOOLS_DIR" ]; then export PATH="$TOOLS_DIR:$PATH"; fi
    APK=$(find mobile/gen/android -name '*.apk' ! -name '*-unsigned*' | head -1)
    if [ -z "$APK" ]; then APK=$(find mobile/gen/android -name '*.apk' | head -1); fi
    if [ -z "$APK" ]; then echo "ERROR: no APK found"; exit 1; fi
    ALIGNED="${APK%.apk}-aligned.apk"
    zipalign -f -v -p 4 "$APK" "$ALIGNED"
    apksigner sign --ks ~/.android/debug.keystore --ks-key-alias androiddebugkey \
        --ks-pass pass:android --key-pass pass:android "$ALIGNED"
    mv -f "$ALIGNED" "mobile/yse-android-universal-release.apk"

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
