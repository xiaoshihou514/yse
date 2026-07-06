# YSE (盐水鹅)

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
├── desktop/    # yse-desktop: Tauri 2 app (tray-icon feature, 16+ commands)
├── mobile/     # yse-mobile: bare Tauri builder, no commands/state (openssl vendored)
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source layout:
- `core/src/identity.rs` — address parse/format (`name#hash@hostname`), hostname, hash generation
- `core/src/plugin/process_manager.rs` — process state machine, crash detection, auto-restart (max 3)
- `core/src/plugin/session.rs` — SessionRegistry: hash→plugin_id routing, per-contact hashes
- `core/src/email/imap.rs` — IMAP poller with Coremail ID workaround
- `core/src/email/smtp.rs` — SMTP send via lettre (tokio1-rustls-tls)
- `core/src/plugin/process.rs` — ManagedPlugin child process lifecycle
- `core/src/plugin/protocol.rs` — PluginRequest / CoreNotification enums
- `core/src/crypto.rs` — Argon2id → ChaCha20-Poly1305
- `core/src/store/sqlite.rs` — SQLite via rusqlite (bundled); `contact_hashes` + `hidden_addresses` tables
- `core/src/event.rs` — broadcast::channel(256) event bus
- `core/src/router.rs` — (legacy, unused; dispatch via SessionRegistry now)
- `desktop/src/lib.rs` — Tauri Builder wiring, temp runtime in .setup()
- `desktop/src/commands.rs` — 22+ Tauri commands + YseState
- `frontend/src/stores/yse.ts` — Pinia store (new state: hostnames, sessions, processes, hiddenAddresses)
- `frontend/src/router/index.ts` — 4 routes: chat/plugins/contacts/config

## Critical gotchas

### IMAP
- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail/QQ Mail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX.
  Uses `session.run_command_and_check_ok(...)` (imap 3.0.0-alpha.15 native support).
- First poll fetches ALL (last_uid starts None). Uses `UID SEARCH ALL` + Rust-side UID filter
  (QQ Mail rejects `UID SEARCH UID N:*`).
- Reconnects every 10s tick — each iteration opens a fresh TCP+TLS connection.

### Setup runtime
- Tauri `.setup()` hook runs **before** Tokio runtime is ready. Use temporary
  `tokio::runtime::Runtime::new()` + `block_on` for one-time init (`desktop/src/lib.rs:35-48`).
- Tasks spawned with `tokio::spawn` inside the temporary runtime are **cancelled** when
  `block_on` returns. Long-lived tasks (IMAP polling started by `start_polling_inner`)
  must use `tokio::spawn` inside a context where Tauri's permanent runtime is active.
- Plugin stdout readers also get cancelled with the temp runtime — plugin communication
  only works if the plugin sends messages synchronously during setup.

### Address format
- All virtual addresses use `name#8char-hex@hostname` format.
- `identity::parse_address(addr)` → `(name, hash, hostname)`
- `identity::local_hostname()` gets system hostname via `hostname` command.
- Per-contact sender hash is persistent (stored in `contact_hashes` SQLite table).
- No more `@yse.org` suffix — hostname is the actual machine hostname.

### Plugin lifecycle (new)
- **No auto-start on boot.** Plugins only start on demand when a message arrives for
  `name#hash@this-hostname`.
- `PluginProcessManager` manages state machine: Stopped↔Starting↔Running↔Stopped/Crashed.
- Crashed plugins auto-restart up to 3 times (detected via stdout reader closure).
- `SessionRegistry::route()` checks hostname match → looks up hash→plugin_id →
  starts plugin if needed → dispatches `CoreNotification::Message`.
- Plugin processes are NOT started for messages addressed to other hostnames.
- `route()` returns `bool` indicating whether local dispatch happened.

### SMTP
- `ContentType::parse("text/plain")` does NOT work via `Header::parse` trait.
  Use `"text/plain".parse::<ContentType>()` (FromStr) or `ContentType::TEXT_PLAIN`.
- SMTP envelope sender must match authenticated user (relay constraint).

### Frontend
- Tauri v2 has **no `__TAURI__` global** — always import from `@tauri-apps/api` and call `invoke`.
- `@` path alias → `src/` (vite.config.ts).
- Dark mode stored in `localStorage` key `"yse-dark"`, applied via `theme-mode` attribute on `<html>`.
- `beforeBuildCommand` in tauri.conf.json skips frontend build in CI: `[ -n "$CI" ] || (cd ../frontend && npm run build)`.
- ChatView: hostname dot nav at top, contact grouping by hostname, hidden conversations section.
- ContactsView: auto-generates `name#hash@hostname` addresses with random hash.

### Plugin system
- Child process JSON-RPC over stdin/stdout. Plugin sends: `send`, `log`.
  Core sends: `message`, `config`, `shutdown`.
- Plugins are **outside** the workspace — compile separately:
  `cd plugins/echo-bot && cargo build`.
- Plugin manager no longer auto-starts; SessionRegistry starts plugins on-demand.

### Encryption
- Argon2id → 32B ChaCha20-Poly1305 key. Fixed salt `b"yse-argon2-salt-v1"`.
- 12B random Nonce prefixed to ciphertext (split at index 12 on decrypt).
- Crypto password IS persisted in config JSON (derived into key on load).

### SQLite
- `dirs_next::data_dir()/yse/yse.db`. Created by `YseState::new(db_path)`.
- Config persisted as single JSON row in `config` table key `"config"`.
- New tables: `contact_hashes(recipient, local_hash)` and `hidden_addresses(address)`.

### Misc
- `just check` / `just clippy` implicitly generate icons from `icon.png` via `tauri icon` and copy `desktop/icons/32x32.png` to `frontend/public/` — do not skip.
- `just android-build` runs `scripts/android-build.sh` (debug keystore, NDK paths, zipalign + apksigner).
- Mobile (`mobile/src/lib.rs`) is a bare Tauri builder — no commands, no plugins, no state.

## CI / Release
- GitHub Actions (`.github/workflows/build.yml`): 4 jobs — desktop (AppImage), android (APK),
  check (core tests + frontend build), release (uploads to GitHub Release `latest` on push to main).
- Flaky step: `android init` has `continue-on-error: true`.
- APK signing uses auto-generated debug keystore.
- Release job runs on push to main after other 3 jobs succeed.
- Don't wait until the CI is done, the android job is extremely slow (~30mins)

## Git conventions
- Commit messages in Chinese, conventional-commits format: `fix:`, `feat:`, `refactor:`, `chore:`, `style:`.
- AI-generated commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Use `kioclient move "file://$path" 'trash:/'` instead of `rm`.
