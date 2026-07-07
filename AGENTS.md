# 盐水鹅

Email-mediated chat + plugin automation system. Tauri 2 desktop (Linux AppImage/deb) + Android (APK via Tauri mobile).

## Quick commands

```sh
cargo check -p yse-core          # fast: core only (no desktop deps)
cargo test -p yse-core           # 16 tests (core only)
cargo clippy -- -D warnings      # lint (also generates icons)
cargo fmt                        # format
just check-all                    # fe-typecheck + cargo check + clippy
just check                        # cargo check (full workspace, generates icons)
just clean                        # cargo clean + rm -rf frontend dist/vite cache

just dev                          # cargo tauri dev (starts Vite + Tauri)
just fe-dev                       # Vite dev server at :1420
just fe-build                     # npm run build (vue-tsc --noEmit && vite build)
just build-appimage               # desktop AppImage bundle
just build-deb                    # desktop deb bundle
just android-init                 # first-time Android project init
just android-build                # full Android APK (frontend + NDK + signing)
just plugin-echo                  # compile plugins/echo-bot separately
just help                         # list all just commands
```

## Repo map

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure logic, no platform deps
├── desktop/    # yse-desktop: Tauri 2 app (tray-icon feature, ~25 commands)
├── mobile/     # yse-mobile: bare Tauri builder, no commands/state (openssl vendored)
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source:
- `desktop/src/lib.rs` — Tauri Builder, plugin registration, temp runtime in .setup()
- `desktop/src/commands.rs` — ~25 Tauri commands + YseState
- `mobile/src/lib.rs` — bare builder, registers tauri-plugin-os + barcode-scanner (`#[cfg(mobile)]`)
- `frontend/src/stores/yse.ts` — Pinia store
- `frontend/src/views/` — 4 views: Chat, Plugins, Contacts, Config
- `frontend/src/composables/useIsMobile.ts` — reactive `isMobile` (width < 768px)
- `scripts/android-build.sh` — full APK build pipeline

## Critical gotchas

### Setup runtime
- Tauri `.setup()` hook runs **before** Tokio runtime is ready. Use temporary
  `tokio::runtime::Runtime::new()` + `block_on` for one-time init (`desktop/src/lib.rs:46-49`).
- Tasks spawned with `tokio::spawn` inside the temporary runtime are **cancelled** when
  `block_on` returns. Long-lived tasks MUST use Tauri's permanent runtime.

### IMAP
- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail/QQ Mail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX
  (`session.run_command_and_check_ok(...)`, imap 3.0.0-alpha.15 native support).
- First poll fetches ALL (last_uid starts None). Uses `UID SEARCH ALL` + Rust-side UID filter.
  QQ Mail rejects `UID SEARCH UID N:*`.

### Frontend
- Tauri v2 has **no `__TAURI__` global** — always import from `@tauri-apps/api`.
- `@` path alias → `src/` (vite.config.ts).
- Theme stored in `localStorage` key `"yse-theme"` (one of `"light"`/`"dark"`/`"auto"`),
  applied via `theme-mode` attribute on `<html>`.
- Use `useIsMobile()` composable for responsive layouts (768px breakpoint).
- Platform detection uses `@tauri-apps/plugin-os` → `platform() === "android"`.
- `beforeBuildCommand` uses timestamp check: skips if `dist/` is newer than all `src/` files.
  Does NOT skip in CI — frontend build runs as part of Tauri build command.
  (`tauri.conf.json` in both `desktop/` and `mobile/`).

### Address format
- All virtual addresses: `name#8char-hex@hostname`.
- `identity::parse_address(addr)` → `(name, hash, hostname)`.
- Per-contact sender hash persistent in `contact_hashes` SQLite table.

### Plugin lifecycle
- **No auto-start on boot.** Plugins start on demand when a message arrives for a local address.
- `SessionRegistry::route()` checks hostname match → hash→plugin_id → starts plugin if needed.
- Crashed plugins auto-restart up to 3 times.

### SMTP
- `ContentType` parsing: use `"text/plain".parse::<ContentType>()` (FromStr), NOT `Header::parse`.
- SMTP envelope sender must match authenticated user.

### Encryption
- Argon2id → 32B ChaCha20-Poly1305 key. Fixed salt `b"yse-argon2-salt-v1"`.
- 12B random Nonce prefixed to ciphertext (split at index 12 on decrypt).

### SQLite
- `dirs_next::data_dir()/yse/yse.db`. Tables: `contact_hashes`, `hidden_addresses`, `config`.

### Plugin system
- Child process JSON-RPC over stdin/stdout. Plugin sends: `send`, `log`.
  Core sends: `message`, `config`, `shutdown`.
- Plugins **outside** workspace — compile with `cd plugins/echo-bot && cargo build`.

## Mobile (Android)

### Build flow
- `just android-build` → `scripts/android-build.sh`:
  1. `rm -rf gen/android icons/android` — force fresh init (picks up plugin native code)
  2. `tauri android init` — generates Android project (must run BEFORE icon gen)
  3. `tauri icon ../icon.png` — generates icons, injects into Android project
  4. Patches `ic_launcher_background.xml` to `#262626` (dark, matches desktop sidebar)
  5. Patches Gradle distribution URL → Tencent Cloud mirror (China)
  6. Creates `~/.gradle/init.gradle` → Aliyun Maven mirrors (bypasses GFW SSL issues)
  7. `tauri android build --apk` → produces unsigned APK
  8. `zipalign` + `apksigner sign` with `mobile/yse-keystore.jks` (committed, persistent key)

### Signing
- Keystore at `mobile/yse-keystore.jks` (RSA 2048, alias=upload, password in `keystore.password`).
- Generated once on first build, committed to repo. All builds use same key → upgrades work.
- If regenerated, commit both files.

### Capabilities (Tauri 2 permission model)
- `mobile/capabilities/default.json` — `core:default` + `os:default` (common, all platforms).
- `mobile/capabilities/mobile.json` — `barcode-scanner:default` (Android/iOS only, via `cargo tauri add barcode-scanner`).
- `desktop/capabilities/default.json` — `core:default`, `shell:allow-open`, `dialog:default`, `os:default`.

### Barcode scanner (camera)
- Only works on Android/iOS (Tauri plugin does not support desktop).
- Dependency in `mobile/Cargo.toml` under `[target.'cfg(android/ios)'.dependencies]`.
- Frontend: `scan({ formats: [Format.QRCode] })` — uses native scanner UI (no `windowed: true`).
- Mobile shows "扫码导入" button, desktop shows "导入配置" (file upload only).
- Plugin's AndroidManifest.xml auto-injects `CAMERA` + `VIBRATE` permissions.

### Gradle mirror (China only)
- `~/.gradle/init.gradle` created by build script — overrides all repos with Aliyun mirrors.
- Without this, `repo.maven.apache.org` fails with SSL cert mismatch (GFW interception).

## Desktop (Linux AppImage)
- Requires: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`,
  `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`.
- `just build-appimage` → `target/release/bundle/appimage/*.AppImage`.

## CI / Release
- GitHub Actions (`.github/workflows/build.yml`): 4 jobs — desktop, android, check, release.
- Android job is extremely slow (~30mins). Don't wait for it.
- Release job runs on push to main after other 3 succeed.
- APK signing uses repo keystore (same key every build).

## Git conventions
- Commit messages in Chinese, conventional-commits format: `fix:`, `feat:`, `refactor:`, `chore:`, `style:`.
- AI-generated commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Author & committer: `xiaoshihou <xiaoshihou@tutamail.com>`.
- Do not use `--author` flag (git config already set).
