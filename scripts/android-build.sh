#!/usr/bin/env bash
set -euo pipefail

# ── Signing configuration ──────────────────────────────────────────
# Keystore is stored in the repo at mobile/yse-keystore.jks so that
# the same key is used for every build (enables app upgrades).
# Generate once on first run — commit the result.
KEYSTORE="$(dirname "$0")/../mobile/yse-keystore.jks"
KEYSTORE_PASSWORD="$(dirname "$0")/../mobile/keystore.password"
KEY_ALIAS="${YSE_KEY_ALIAS:-upload}"

if [ ! -f "$KEYSTORE" ]; then
    echo "Generating new keystore at ${KEYSTORE} ..."
    PASSWORD="${YSE_KEY_PASSWORD:-yse-android-sign}"
    echo "$PASSWORD" > "$KEYSTORE_PASSWORD"
    keytool -genkey -v -keystore "$KEYSTORE" \
        -alias "$KEY_ALIAS" -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass "$PASSWORD" -keypass "$PASSWORD" \
        -dname "CN=YSE,O=YSE,C=CN"
    echo "Keystore created. Commit ${KEYSTORE} and ${KEYSTORE_PASSWORD} to repo."
fi

KEY_PASSWORD=$(cat "$KEYSTORE_PASSWORD")

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
# 3. patch Android adaptive icon background to dark (like desktop sidebar)
for dir in icons/android gen/android/app/src/main/res; do
  mkdir -p "$dir/values"
  cat > "$dir/values/ic_launcher_background.xml" << 'XML'
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">#262626</color>
</resources>
XML
done
# 4. copy 32x32 for frontend/public
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
    APK=$(find mobile/gen/android -name '*.apk' ! -name '*-unsigned*' | head -1)
fi
if [ -z "$APK" ]; then echo "ERROR: no APK found"; exit 1; fi

ALIGNED="${APK%.apk}-aligned.apk"
zipalign -f -v -p 4 "$APK" "$ALIGNED"

apksigner sign --ks "$KEYSTORE" --ks-key-alias "$KEY_ALIAS" \
    --ks-pass pass:"$KEY_PASSWORD" --key-pass pass:"$KEY_PASSWORD" "$ALIGNED"
echo "APK signed with alias '${KEY_ALIAS}' from repo keystore"

mv -f "$ALIGNED" "mobile/yse-android-universal-release.apk"
