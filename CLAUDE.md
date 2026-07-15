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
just check                        # cargo check full workspace + icon gen + fe-typecheck
just dev                          # cd desktop && cargo tauri dev
just fe-dev                       # Vite dev server at :1420
just android                      # bash scripts/android-build.sh (full APK)
just appimage                     # desktop AppImage
just plugin-all                   # build all plugins (echo-bot, file-tree, pm, opencode-bot)
just format                       # prettier + rustfmt
```

## Repo

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure logic, no platform deps
├── desktop/    # yse-desktop: Tauri 2 app (27 commands + YseState)
├── mobile/     # yse-mobile: bare Tauri (15 commands, no plugin commands)
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source: `desktop/src/commands.rs` (YseState + 27 commands), `desktop/src/lib.rs` (builder + temp runtime),
`mobile/src/lib.rs` (uses `app.path().app_data_dir()` — NOT `dirs_next`), `core/src/app.rs` (CoreState),
`core/src/imap_ingest.rs` (shared ingest logic + `classify` for address matching),
`frontend/src/stores/yse.ts` (Pinia store, skips plugin/session APIs on Android via `platform() === "android"`),
`frontend/src/views/` (Chat, Plugins, Contacts, Config),
`frontend/src/utils/address.ts` (parseAddress, hostnameFromAddr, nameFromAddr),
`scripts/android-build.sh` (APK pipeline), `plugins/opencode-bot/opencode-tools/bash.ts` (custom bash tool).

## Gotchas

### Address matching — most common bug

`config.own_address` is just `me` (hardcoded, overridden from "me" in load_config).
`format_sender_address(recipient)` produces `name#8char-hex@hostname`. **Never `==` against message addresses.**
- Rust: `addr.starts_with(&format!("{}#", own))` or `addr == own` (bare fallback) — see `imap_ingest.rs::classify`.
- Frontend: `nameFromAddr(addr)` from `@/utils/address` → compare against `ownAddress.value`.

Every comparison in `ChatView.vue`, `commands.rs` (new-message emit, route skip), and `imap_ingest.rs` (for_self check) must use name-based matching.

### `is_processed` semantics

- `processed` column defaults to `0`. `save_message()` does NOT set it.
- IMAP poll checks `is_processed` before routing. To prevent re-route from SMTP copy, call `mark_processed(msg.id)` after any local `route()` call.

### IMAP echo loop prevention

Plugin→user messages MUST skip `route()` in IMAP callback. Handled by `ingest_core` in `imap_ingest.rs` — if `for_self` (to_addr is us), skip routing and mark processed. Without this, PluginNotFound triggers error reply → plugin echoes → infinite loop.

### `send_message` flow (order matters)

`send_message` Tauri command:
1. `save_message` — persist to DB
2. `route` — deliver to local plugin if addressed here
3. SMTP send — external delivery
4. `mark_processed` — prevent IMAP from re-routing the SMTP copy (only after successful send)

Same order applies in plugin Send handler.

### Plugin `virtual_addr`

`CoreNotification::Config` is sent to plugins on startup with `virtual_addr: None`.
The plugin gets its actual virtual address (`name#hash@hostname`) only after a session is registered in `SessionRegistry::resolve_plugin()`, which pushes an updated Config with the virtual_addr.

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
- Android hostname: kernel always returns `"localhost"`. `resolveHostname()` in `stores/yse.ts` falls back to device model from userAgent or persistent `localStorage` ID.
- `npm run build` runs `vue-tsc --noEmit` first (type-check gates the build).
- `m.timestamp` from Rust is `as_millis()` (ms). `Date.now()` on frontend is also ms.
  `readTimestamps` stores `Date.now()` (ms) — comparison against `m.timestamp` is ms vs ms.
  Do NOT divide by 1000, do NOT compare against `as_secs()`.

### Plugin system

- Child processes, JSON-RPC over stdin/stdout. Plugin sends `send`/`log`. Core sends `message`/`config`/`shutdown`.
- No auto-start on boot. `SessionRegistry::route()` starts plugin on demand.
- Crashed plugins auto-restart up to 3 times.
- Plugins outside workspace: `plugins/echo-bot` (Rust), `plugins/opencode-bot` (TypeScript), `plugins/file-tree` (Rust), `plugins/project-manager` (Rust + rig-core + Ollama).

### exec 工具

内置 `bash` 已被自定义 `exec` tool 替代（源码 `plugins/opencode-bot/opencode-tools/exec.ts`）：
- 所有命令都通过 tmux session 执行，session 自动创建和管理
- session 隔离：每个 OpenCode 会话独立 tmux socket `/tmp/yse-tmux/yse-<sessionID>.sock`
- 支持 SSH 远程执行（`server` 参数）；2 分钟无变化时返回部分输出
- `just plugin-opencode` 编译插件并复制 exec.ts 到 `.opencode/tools/`
- **重要：不要手动创建 tmux session。exec 工具已内置所有 tmux 逻辑。**

### SSH quoting

SSH 路径通过 `spawnSync("ssh", [..., "tmux", ...args])` 执行。SSH 将参数拼接为远程 shell 命令，`;` 等元字符会破坏参数边界。
修复方式：SSH 时对每个 tmux 参数做 `shQuote`（单引号包裹），然后作为一个字符串发送。
容器内需要先 `mkdir -p <SOCKET_DIR>` 否则 tmux new-session 失败。

## Mobile (Android)

- `just android` → `scripts/android-build.sh`: icon gen → patch launcher bg (`#262626`) → Gradle mirror patch → `tauri android build --apk` → `zipalign` + `apksigner sign`.
- Keystore: `mobile/yse-keystore.jks` (RSA 2048, alias=upload, password in `keystore.password`). Generated on first run.
- `~/.gradle/init.gradle` overrides all repos with Aliyun mirrors (GFW SSL issue).
- Capabilities: `mobile/capabilities/default.json` (core+os), `mobile/capabilities/mobile.json` (barcode-scanner).
- Mobile lib uses `app.path().app_data_dir()` (not `dirs_next`) to get storage path on Android.
- Mobile has **no plugin commands** (no route/dispatch; uses `ingest_message` from core).
- Barcode scanner: mobile shows "扫码导入", desktop shows "导入配置" (file upload).

## Desktop

Requires: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`.

## CI / Git

- `.github/workflows/build.yml.disabled`: 4 jobs (desktop, android, check, release). Currently disabled.
- Commits: Chinese, conventional-commits (`fix:`/`feat:`/`refactor:`/`chore:`/`style:`).
- AI commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Author: `xiaoshihou <xiaoshihou@tutamail.com>`. Do not use `--author` flag.
