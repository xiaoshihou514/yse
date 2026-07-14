# 盐水鹅

Email-mediated chat + plugin automation system. Tauri 2 desktop (AppImage/deb) + Android (APK).

See `agent_docs/` for architecture deep-dive.

## Quick commands

```sh
cargo check -p yse-core          # fast: core only (no desktop deps)
cargo test -p yse-core           # 25 tests
cargo clippy -- -D warnings      # lint (generates icons + copies to frontend)
cargo fmt --all
just fe-typecheck                 # vue-tsc --noEmit
just fe-build                     # npm run build (runs vue-tsc first)
just check-all                    # fe-typecheck + cargo check + clippy
just check                        # cargo check (full workspace + icon gen)
just dev                          # cd desktop && cargo tauri dev
just fe-dev                       # Vite dev server at :1420
just android-build                # bash scripts/android-build.sh (full APK)
just build-appimage               # desktop AppImage
```

## Repo

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure logic, no platform deps
├── desktop/    # yse-desktop: Tauri 2 app (28 commands + YseState)
├── mobile/     # yse-mobile: bare Tauri (14 commands, no plugins)
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source: `desktop/src/commands.rs` (YseState + 28 commands), `desktop/src/lib.rs` (builder + temp runtime),
`mobile/src/lib.rs` (uses `app.path().app_data_dir()` — NOT `dirs_next`), `core/src/app.rs` (CoreState),
`core/src/imap_ingest.rs` (shared ingest logic + `classify` for address matching),
`frontend/src/stores/yse.ts` (Pinia store, skips plugin/session APIs on Android via `platform() === "android"`),
`frontend/src/views/` (Chat, Plugins, Contacts, Config),
`frontend/src/utils/address.ts` (parseAddress, hostnameFromAddr, nameFromAddr),
`scripts/android-build.sh` (APK pipeline).

## Gotchas

### Address matching — most common bug

`config.own_address` is just `me` (hardcoded, overridden from "me" in load_config).
`format_sender_address(recipient)` produces `name#8char-hex@hostname`. **Never `==` against message addresses.**
- Rust: `addr.starts_with(&format!("{}#", own))` or `addr == own` (bare fallback) — see `imap_ingest.rs:classify`.
- Frontend: `nameFromAddr(addr)` from `@/utils/address` → compare against `ownAddress.value`.

Every comparison in `ChatView.vue`, `commands.rs` (new-message emit, route skip), and `imap_ingest.rs` (for_self check) must use name-based matching.

### `is_processed` semantics

- `processed` column defaults to `0`. `save_message()` does NOT set it.
- IMAP poll checks `is_processed` before routing. To prevent re-route from SMTP copy, call `mark_processed(msg.id)` after any local `route()` call.
- **Mobile bug**: `mark_processed` is called **before** `save_message` in mobile IMAP callback (`mobile/src/commands.rs:230-233`). UPDATE hits 0 rows, then INSERT creates row with `processed=0`. Harmless on mobile (no plugins/route), but bad pattern to copy.

### IMAP echo loop prevention

Plugin→user messages MUST skip `route()` in IMAP callback. Handled by `ingest_core` in `imap_ingest.rs` — if `for_self` (to_addr is us), skip routing and mark processed. Without this, PluginNotFound triggers error reply → plugin echoes → infinite loop.

### `send_message` flow (order matters)

`send_message` Tauri command (`desktop/src/commands.rs:224`):
1. `save_message` — persist to DB
2. `route` — deliver to local plugin if addressed here
3. `mark_processed` — prevent IMAP from re-routing the SMTP copy
4. SMTP send — external delivery

Same order applies in plugin Send handler: save_message → mark_processed → then SMTP send.

### Plugin `virtual_addr`

`CoreNotification::Config` is sent to plugins on startup with `virtual_addr: None`.
The plugin gets its actual virtual address (`name#hash@hostname`) only after a session is registered in `SessionRegistry::resolve_plugin()` (`session.rs:159`), which pushes an updated Config with the virtual_addr.

A plugin that sends a welcome message before receiving any user message will have empty `from`/`to` addresses. The plugin Send handler in `commands.rs` detects empty `to_addr` and saves the message locally **without** sending via SMTP. `ingest_core` in `imap_ingest.rs` also skips routing for messages with empty `to_addr`.

### Setup runtime

Tauri `.setup()` runs before Tokio runtime. Use temporary `tokio::runtime::Runtime::new()` + `block_on` for one-time init. Tasks spawned inside are cancelled when `block_on` returns — long-lived tasks (IMAP polling, plugin stdout readers) use Tauri's permanent runtime (spawned from Tauri commands).

### IMAP

- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail/QQ Mail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX.
- First poll uses `UID SEARCH ALL` (last_uid starts None) + Rust-side filter. QQ Mail rejects `UID SEARCH UID N:*`.
- `parse_address` (Rust) returns `None` for addresses without `#` (e.g. bare `me@yse.org`).

### Frontend

- Tauri v2 has **no `__TAURI__` global** — import from `@tauri-apps/api`.
- `@` path alias → `src/` (vite.config.ts).
- Theme: `localStorage` key `"yse-theme"` → `"light"`/`"dark"`/`"auto"`, applied via `theme-mode` attr on `<html>`.
- Platform: `@tauri-apps/plugin-os` → `platform() === "android"`.
- `beforeBuildCommand` skips if `dist/` newer than all `src/` files (timestamp check). Does NOT skip in CI.
- Android hostname: kernel always returns `"localhost"`. `resolveHostname()` in `stores/yse.ts:29` falls back to device model from userAgent or persistent `localStorage` ID.
- `npm run build` runs `vue-tsc --noEmit` first (type-check gates the build).
- `m.timestamp` from Rust is `as_millis()` (ms, `core/src/message.rs:48`). `Date.now()` on frontend is also ms.
  `readTimestamps` stores `Date.now()` (ms) — comparison against `m.timestamp` is ms vs ms.
  Do NOT divide by 1000, do NOT compare against `as_secs()`.

### Plugin system

- Child processes, JSON-RPC over stdin/stdout. Plugin sends `send`/`log`. Core sends `message`/`config`/`shutdown`.
- No auto-start on boot. `SessionRegistry::route()` starts plugin on demand.
- Crashed plugins auto-restart up to 3 times.
- Plugins outside workspace: `plugins/echo-bot` (Rust), `plugins/opencode-bot` (TypeScript), `plugins/file-tree` (Rust), `plugins/project-manager` (Rust + rig-core + Ollama).

### bash 工具

内置 `bash` 已被自定义 bash tool 替代（源码 `plugins/opencode-bot/opencode-tools/bash.ts`）：
- 短命令（`cd` / `ls` / `grep` / `cat` 等）直接执行并返回结果
- 长命令通过 tmux marker-based 算法执行：`clear;echo START → cmd;echo END → 精确截取输出`
- session 隔离：每个 OpenCode 会话独立 tmux socket `/tmp/yse-tmux/yse-<sessionID>.sock`
- 支持 SSH 远程执行（`server` 参数）
- 进度检测：2 分钟无变化时返回部分输出
- `just plugin-opencode` 会自动编译插件并复制 tool 到 `.opencode/tools/`

## Mobile (Android)

- `just android-build` → `scripts/android-build.sh`: icon gen → patch launcher bg (`#262626`) → Gradle mirror patch → `tauri android build --apk` → `zipalign` + `apksigner sign`.
- Keystore: `mobile/yse-keystore.jks` (RSA 2048, alias=upload, password in `keystore.password`). Generated on first run.
- `~/.gradle/init.gradle` overrides all repos with Aliyun mirrors (GFW SSL issue).
- Capabilities: `mobile/capabilities/default.json` (core+os), `mobile/capabilities/mobile.json` (barcode-scanner).
- Mobile lib uses `app.path().app_data_dir()` (not `dirs_next`) to get storage path on Android.
- Mobile has **no plugin commands** (`mobile/src/commands.rs` has 14 commands vs desktop's 28).
- Barcode scanner: mobile shows "扫码导入", desktop shows "导入配置" (file upload).

## Desktop

Requires: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`.

## CI / Git

- `.github/workflows/build.yml`: 4 jobs (desktop, android, check, release).
- Android job needs JDK 17 + Android SDK. Pipeline may lack credits.
- Commits: Chinese, conventional-commits (`fix:`/`feat:`/`refactor:`/`chore:`/`style:`).
- AI commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Author: `xiaoshihou <xiaoshihou@tutamail.com>`. Do not use `--author` flag.
