#!/usr/bin/env bash
set -euo pipefail

# ── Signing configuration ──────────────────────────────────────────
# Priority: env vars > ~/.android/yse-upload-keystore.jks
# In CI: set YSE_KEYSTORE_BASE64 + YSE_KEY_PASSWORD + YSE_KEY_ALIAS
# Locally: create ~/.android/yse-upload-keystore.jks once and set
#   YSE_KEY_PASSWORD (or store password in KEYSTORE_PASSWORD env).
KEYSTORE="${YSE_KEYSTORE:-$HOME/.android/yse-upload-keystore.jks}"
KEY_PASSWORD="${YSE_KEY_PASSWORD:-}"
KEY_ALIAS="${YSE_KEY_ALIAS:-upload}"

# CI mode: decode base64 keystore from secret
if [ -n "${YSE_KEYSTORE_BASE64:-}" ]; then
    KEYSTORE=$(mktemp)
    base64 -d <<< "$YSE_KEYSTORE_BASE64" > "$KEYSTORE"
fi

# ── NDK setup ──────────────────────────────────────────────────────
NDK_DIR=$(ls -1d "${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/ndk/"*/ 2>/dev/null | head -1)
if [ -n "$NDK_DIR" ]; then
    export TARGET_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
    export PATH="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
fi

cd mobile

# Remove old Android project to force fresh init (so barcode-scanner
# plugin's native code / permissions are picked up)
rm -rf gen/android icons/android

# 1. init MUST run before icon so tauri icon can inject into the project
npx @tauri-apps/cli@^2 android init
# 2. generate icons — detects existing Android project and places
#    icons into gen/android/app/src/main/res/mipmap-* directly
npx @tauri-apps/cli@^2 icon ../icon.png
# 3. copy 32x32 for frontend/public
cp -f icons/32x32.png ../frontend/public/icon.png

npm install
npx @tauri-apps/cli@^2 android build --apk
cd ..

# ── find & sign APK ────────────────────────────────────────────────
BUILD_TOOLS="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/build-tools"
TOOLS_DIR=$(ls -1d "$BUILD_TOOLS"/*/ 2>/dev/null | head -1)
if [ -n "$TOOLS_DIR" ]; then export PATH="$TOOLS_DIR:$PATH"; fi

APK=$(find mobile/gen/android -name '*-unsigned.apk' | head -1)
if [ -z "$APK" ]; then
    # fallback: signed APK
    APK=$(find mobile/gen/android -name '*.apk' ! -name '*-unsigned*' | head -1)
fi
if [ -z "$APK" ]; then echo "ERROR: no APK found"; exit 1; fi

ALIGNED="${APK%.apk}-aligned.apk"
zipalign -f -v -p 4 "$APK" "$ALIGNED"

if [ -f "$KEYSTORE" ] && [ -n "$KEY_PASSWORD" ]; then
    apksigner sign --ks "$KEYSTORE" --ks-key-alias "$KEY_ALIAS" \
        --ks-pass pass:"$KEY_PASSWORD" --key-pass pass:"$KEY_PASSWORD" "$ALIGNED"
    echo "APK signed with alias '${KEY_ALIAS}'"
elif [ -f "$KEYSTORE" ] && [ -z "$KEY_PASSWORD" ]; then
    echo "WARNING: keystore found at ${KEYSTORE} but YSE_KEY_PASSWORD is not set"
    echo "APK will NOT be properly signed — upgrade will fail!"
else
    echo "WARNING: no keystore found, APK left unsigned"
    echo "To fix locally:"
    echo "  1. keytool -genkey -v -keystore ~/.android/yse-upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload"
    echo "  2. export YSE_KEY_PASSWORD=your-password"
    echo "  3. run this script again"
    echo "In CI: set YSE_KEYSTORE_BASE64, YSE_KEY_PASSWORD, YSE_KEY_ALIAS secrets"
fi

mv -f "$ALIGNED" "mobile/yse-android-universal-release.apk"
