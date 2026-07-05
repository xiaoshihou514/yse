#!/usr/bin/env bash
set -euo pipefail

mkdir -p ~/.android
KEYSTORE=~/.android/debug.keystore
if [ ! -f "$KEYSTORE" ]; then
    keytool -genkey -v -keystore "$KEYSTORE" \
        -alias androiddebugkey -storepass android -keypass android \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -dname "CN=Android Debug,O=Android,C=US"
fi

NDK_DIR=$(ls -1d "${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/ndk/"*/ 2>/dev/null | head -1)
if [ -n "$NDK_DIR" ]; then
    export TARGET_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
    export CARGO_TARGET_AARCH64_LINUX_ANDROID_RANLIB="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib"
    export PATH="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
fi

cd mobile
npx @tauri-apps/cli@^2 icon ../icon.png
cp icons/32x32.png ../frontend/public/icon.png
npx @tauri-apps/cli@^2 android init || true
npm install
npx @tauri-apps/cli@^2 android build --apk
cd ..

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
