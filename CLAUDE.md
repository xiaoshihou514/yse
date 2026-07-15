# 盐水鹅

Email-mediated chat + plugin automation system. Tauri 2 desktop (AppImage/deb) + Android (APK).

## Quick commands

```sh
cargo check -p yse-core          # fast: core only (no desktop deps)
cargo test -p yse-core           # 25 tests
cargo clippy -- -D warnings      # lint (generates icons + copies to frontend)
cargo fmt --all
just check                        # cargo check full workspace + icon gen + fe-typecheck
just dev                          # cd desktop && cargo tauri dev
just android                      # bash scripts/android-build.sh (full APK)
just appimage                     # desktop AppImage
just plugin-all                   # build all plugins (echo-bot, file-tree, pm, opencode-bot)
just format                       # prettier + rustfmt
```

## Repo

Cargo workspace (resolver = "2"): `core/`, `desktop/`, `mobile/`. Plugins outside workspace in `plugins/`.

Key source: `desktop/src/commands.rs` (27 Tauri commands + YseState), `desktop/src/lib.rs` (Tauri builder + temp tokio runtime for setup), `mobile/src/lib.rs` (uses `app.path().app_data_dir()` — NOT `dirs_next`), `core/src/app.rs` (CoreState), `core/src/imap_ingest.rs` (shared ingest + `classify` for address matching), `frontend/src/stores/yse.ts` (Pinia store), `frontend/src/utils/address.ts` (parseAddress, hostnameFromAddr, nameFromAddr), `plugins/opencode-bot/opencode-tools/exec.ts` (custom shell tool).

## Address matching — most common bug

`config.own_address` is hardcoded to `"me"`. `format_sender_address(recipient)` produces `name#8char-hex@hostname`. **Never `==` against message addresses.**
- Rust: `addr.starts_with(&format!("{}#", own))` or `addr == own` (bare fallback) — see `imap_ingest.rs::classify`.
- Frontend: `nameFromAddr(addr)` from `@/utils/address` → compare against `ownAddress.value`.

Every comparison in `ChatView.vue`, `commands.rs` (new-message emit, route skip), and `imap_ingest.rs` (for_self check) must use name-based matching.

## `is_processed` semantics

- `processed` column defaults to `0`. `save_message()` does NOT set it.
- IMAP poll checks `is_processed` before routing. Call `mark_processed(msg.id)` after any local `route()` call to prevent re-route from SMTP copy.

## IMAP echo loop prevention

Plugin→user messages MUST skip `route()` in IMAP callback. Handled by `ingest_core` in `imap_ingest.rs` — if `for_self` (to_addr is us), skip routing and mark processed.

## `send_message` flow (order matters)

1. `save_message` — persist to DB
2. `route` — deliver to local plugin if addressed here
3. SMTP send — external delivery
4. `mark_processed` — prevent IMAP from re-routing the SMTP copy (only after successful send)

Same order in plugin Send handler.

## Plugin `virtual_addr`

Plugin gets its virtual address (`name#hash@hostname`) only after a session is registered in `SessionRegistry::resolve_plugin()`. Before that, `CoreNotification::Config` has `virtual_addr: None`. Plugin Send handler detects empty `to_addr` and saves locally without SMTP.

## Setup runtime

Tauri `.setup()` runs before Tokio runtime. Use temporary `tokio::runtime::Runtime::new()` + `block_on` for one-time init. Tasks spawned inside are cancelled when `block_on` returns. Long-lived tasks (IMAP polling, plugin stdout readers) use Tauri's permanent runtime (spawned from Tauri commands).

## IMAP

- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail/QQ Mail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX.
- First poll uses `UID SEARCH ALL` (last_uid starts None) + Rust-side filter. QQ Mail rejects `UID SEARCH UID N:*`.
- `parse_address` (Rust) returns `None` for addresses without `#` (e.g. bare `me@yse.org`).

## Frontend

- Tauri v2 has **no `__TAURI__` global** — import from `@tauri-apps/api`.
- `@` path alias → `src/` (vite.config.ts).
- Theme: `localStorage` key `"yse-theme"` → `"light"`/`"dark"`/`"auto"`, applied via `theme-mode` attr on `<html>`.
- Platform: `@tauri-apps/plugin-os` → `platform() === "android"`.
- `npm run build` runs `vue-tsc --noEmit` first (type-check gates the build).
- Android hostname: kernel returns `"localhost"`. `resolveHostname()` in `stores/yse.ts` falls back to device model from userAgent or persistent `localStorage` ID.
- `m.timestamp` from Rust is `as_millis()` (ms). `Date.now()` on frontend is also ms. Do NOT divide by 1000, do NOT compare against `as_secs()`.

## Plugin system

- Child processes, JSON-RPC over stdin/stdout. Plugin sends `send`/`log`. Core sends `message`/`config`/`shutdown`.
- No auto-start. `SessionRegistry::route()` starts plugin on demand. Crashed plugins auto-restart up to 3 times.
- Plugins outside workspace: `plugins/echo-bot` (Rust), `plugins/opencode-bot` (TypeScript + OpenCode SDK), `plugins/file-tree` (Rust), `plugins/project-manager` (Rust + rig-core + Ollama).

## bash 工具（自定义，覆盖内置 bash）

内置 `bash` 已被项目级自定义 bash tool 替代（源码 `plugins/opencode-bot/opencode-tools/exec.ts`）：
- 安装为 `.opencode/tools/bash.ts`（同名覆盖内置 bash），所有命令通过 tmux 执行
- session 隔离：每个 OpenCode 会话独立 tmux socket `/tmp/yse-tmux/yse-<sessionID>.sock`
- 支持 SSH 远程执行（`server` 参数）；2 分钟无变化时返回部分输出
- `just plugin-opencode` 编译插件并复制 bash.ts 到 `.opencode/tools/`
- **不要手动创建 tmux session。bash 工具已内置所有 tmux 逻辑。**

## SSH quoting

`spawnSync("ssh", [..., "tmux", ...args])` 通过 SSH 将参数拼接为远程 shell 命令，`;` 等元字符会破坏参数边界。
修复：SSH 时对每个 tmux 参数做 `shQuote`（单引号包裹），合并为一个字符串发送。
容器内需先 `mkdir -p <SOCKET_DIR>` 否则 tmux new-session 失败。

## Mobile (Android)

- `just android` → `scripts/android-build.sh`: icon gen → patch launcher bg (`#262626`) → Gradle mirror patch → `tauri android build --apk` → `zipalign` + `apksigner sign`.
- Keystore: `mobile/yse-keystore.jks` (RSA 2048, alias=upload).
- `~/.gradle/init.gradle` overrides all repos with Aliyun mirrors (GFW SSL issue).
- Mobile lib uses `app.path().app_data_dir()` (not `dirs_next`) for storage path.
- Mobile has **no plugin commands** (no route/dispatch; uses `ingest_message` from core).
- Barcode scanner: mobile shows "扫码导入", desktop shows "导入配置" (file upload).

## Desktop

Requires: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`.

## CI / Git

- `.github/workflows/build.yml.disabled`: 4 jobs (desktop, android, check, release). Currently disabled.
- Commits: Chinese, conventional-commits (`fix:`/`feat:`/`refactor:`/`chore:`/`style:`).
- AI commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Author: `xiaoshihou <xiaoshihou@tutamail.com>`. Do not use `--author` flag.
