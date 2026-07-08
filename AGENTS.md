# 盐水鹅

Email-mediated chat + plugin automation system. Tauri 2 desktop (AppImage/deb) + Android (APK).

## Quick commands

```sh
cargo check -p yse-core          # fast: core only (no desktop deps)
cargo test -p yse-core           # 16 tests
cargo clippy -- -D warnings      # lint (also generates icons)
cargo fmt
just check-all                    # fe-typecheck + cargo check + clippy
just check                        # cargo check (full workspace, generates icons)
just dev                          # cargo tauri dev
just fe-dev                       # Vite dev server at :1420
just fe-build                     # npm run build
just android-build                # full APK (frontend + NDK + signing)
just build-appimage               # desktop AppImage
just help                         # list all just commands
```

## Repo

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure logic, no platform deps
├── desktop/    # yse-desktop: Tauri 2 app (~25 commands + YseState)
├── mobile/     # yse-mobile: bare Tauri, no commands/state
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source: `desktop/src/commands.rs` (YseState + commands), `desktop/src/lib.rs` (builder + temp runtime),
`frontend/src/stores/yse.ts` (Pinia store), `frontend/src/views/` (Chat, Plugins, Contacts, Config),
`frontend/src/utils/address.ts` (parseAddress, hostnameFromAddr, nameFromAddr),
`scripts/android-build.sh` (APK pipeline).

## Gotchas

### Address matching — most common bug

`config.own_address` is just the name part (e.g. `me@yse.org`), but `format_sender_address(recipient)`
produces `name#8char-hex@hostname`. **Never `==` against message addresses.** Use:
- Rust: `addr.starts_with(&format!("{}#", own))` or `addr == own` (bare fallback)
- Frontend: `nameFromAddr(addr)` from `@/utils/address` → compare against `ownAddress.value`

Every comparison in `ChatView.vue` (bubble side, contacts dedup, conversation filter, component response)
and `commands.rs` (new-message emit, `for_self` check, send_message route skip) must use name-based
matching, not exact address comparison.

### `is_processed` semantics

- `processed` column defaults to `0`. `save_message()` does NOT set it.
- IMAP poll checks `is_processed` before routing. To prevent re-route from SMTP copy, call
  `mark_processed(msg.id)` after any local `route()` call (see `send_message` in `commands.rs`).
- **Mobile bug**: `mark_processed` is called **before** `save_message` in IMAP callback
  (`mobile/src/commands.rs:227-228`). UPDATE hits 0 rows, then INSERT creates row with `processed=0`.
  Harmless on mobile (no plugins/route), but bad pattern to copy.

### IMAP echo loop prevention

Plugin→user messages MUST skip `route()` in IMAP callback. The `for_self` check in
`desktop/src/commands.rs:548` catches `to_addr == own || starts_with("name#")`. Without this,
PluginNotFound triggers error reply → plugin echoes → infinite loop.

### `send_message` flow (order matters)

`desktop/src/commands.rs` `send_message` Tauri command:
1. `save_message` — persist to DB
2. `route` — deliver to local plugin if addressed here
3. `mark_processed` — prevent IMAP from re-routing the SMTP copy
4. SMTP send — external delivery

Same order applies in plugin Send handler (`PluginRequest::Send`): save_message BEFORE SMTP send.

### Setup runtime

Tauri `.setup()` runs before Tokio runtime. Use temporary `tokio::runtime::Runtime::new()` +
`block_on` for one-time init (`desktop/src/lib.rs:46-49`). Tasks spawned inside are cancelled when
`block_on` returns — long-lived tasks use Tauri's permanent runtime.

### IMAP

- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail/QQ Mail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX.
- First poll uses `UID SEARCH ALL` (last_uid starts None) + Rust-side filter.
  QQ Mail rejects `UID SEARCH UID N:*`.
- `parse_address` (Rust) returns `None` for addresses without `#` (e.g. bare `me@yse.org`).

### Frontend

- Tauri v2 has **no `__TAURI__` global** — import from `@tauri-apps/api`.
- `@` path alias → `src/` (vite.config.ts).
- Theme: `localStorage` key `"yse-theme"` → `"light"`/`"dark"`/`"auto"`, applied via
  `theme-mode` attr on `<html>`.
- Platform: `@tauri-apps/plugin-os` → `platform() === "android"`.
- `beforeBuildCommand` skips if `dist/` newer than all `src/` files (timestamp check).
  Does NOT skip in CI.

### Plugin system

- Child processes, JSON-RPC over stdin/stdout. Plugin sends `send`/`log`. Core sends `message`/`config`/`shutdown`.
- No auto-start on boot. `SessionRegistry::route()` starts plugin on demand.
- Crashed plugins auto-restart up to 3 times.
- Plugins outside workspace: `plugins/echo-bot` (Rust), `plugins/opencode-bot` (TypeScript), `plugins/file-tree` (Rust).

## Mobile (Android)

- `just android-build` → `scripts/android-build.sh`: fresh init → gen icons → patch launcher bg (`#262626`)
  → Gradle mirror patch → `tauri android build --apk` → `zipalign` + `apksigner sign`.
- Keystore: `mobile/yse-keystore.jks` (RSA 2048, alias=upload, password in `keystore.password`).
- `~/.gradle/init.gradle` overrides all repos with Aliyun mirrors (GFW SSL issue).
- Capabilities: `mobile/capabilities/default.json` (core+os), `mobile/capabilities/mobile.json` (barcode-scanner).
- Barcode scanner: mobile shows "扫码导入", desktop shows "导入配置" (file upload).

## Desktop

Requires: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`,
`libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`.

## CI / Git

- `.github/workflows/build.yml`: 4 jobs (desktop, android, check, release). Android ~30min. Pipeline may lack credits.
- Commits: Chinese, conventional-commits (`fix:`/`feat:`/`refactor:`/`chore:`/`style:`).
- AI commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Author: `xiaoshihou <xiaoshihou@tutamail.com>`. Do not use `--author` flag.
