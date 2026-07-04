# YSE (盐水鹅)

Email-mediated chat + plugin automation system. Tauri 2 desktop (Linux AppImage/deb) + Android (APK via Tauri mobile).

## Quick commands

```sh
cargo check -p yse-core          # fast: core only
cargo test -p yse-core           # 9 tests (core only, no desktop deps)
cargo clippy -- -D warnings      # lint
cargo fmt                        # format
just check-all                    # fe-typecheck + cargo check

just dev                          # cargo tauri dev (starts Vite + Tauri)
just fe-dev                       # Vite dev server at :1420
just fe-build                     # npm run build (vue-tsc --noEmit && vite build)
just build-appimage               # desktop AppImage bundle
just build-deb                    # desktop deb bundle
just android-init                 # first-time Android project init
just android-build                # Android APK build
just plugin-echo                  # compile plugins/echo-bot separately
```

## Repo map

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure logic, no platform deps
├── desktop/    # yse-desktop: Tauri 2 app entrypoint
├── mobile/     # yse-mobile: Tauri 2 Android stub (openssl vendored)
├── frontend/   # Vue 3 + Pinia + TDesign + vue-router
└── plugins/    # standalone executables, outside workspace
```

Key source layout:
- `core/src/email/imap.rs` — IMAP poller + unsafe Coremail ID workaround
- `core/src/email/smtp.rs` — SMTP send via lettre
- `core/src/plugin/process.rs` — child process JSON-RPC lifecycle
- `core/src/plugin/protocol.rs` — PluginRequest / CoreNotification enums
- `core/src/crypto.rs` — Argon2id → ChaCha20-Poly1305
- `core/src/store/sqlite.rs` — SQLite via rusqlite (bundled)
- `desktop/src/lib.rs` — Tauri Builder wiring, temp runtime in .setup()
- `desktop/src/commands.rs` — 15 Tauri commands + YseState, log_emit, plugin_handler
- `frontend/src/stores/yse.ts` — Pinia store (actions call @tauri-apps/api invoke)
- `frontend/src/router/index.ts` — 5 routes: chat/plugins/contacts/config/logs

## Critical gotchas

### IMAP
- `imap::Session` is **not `Send`** — never hold across `.await`.
- 163/Coremail requires `ID ("name" "yse" "version" "1.0")` before SELECT INBOX.
  `imap_proto` cannot parse `* ID (...)` response, so `send_id_raw` uses `unsafe` transmute
  to `BufStream` for manual write/read (`core/src/email/imap.rs:120-137`).
- `imap` v2 uses `native-tls` (runtime OpenSSL). `imap-proto` future-compat warning is harmless.
- First poll fetches ALL (last_uid starts None). Uses `SEARCH ALL` + Rust-side UID filter
  (some servers reject `UID SEARCH UID N:*`).

### Setup runtime
- Tauri `.setup()` hook runs **before** Tokio runtime is ready. Use temporary
  `tokio::runtime::Runtime::new()` + `block_on` for one-time init (`desktop/src/lib.rs:35-48`).
- Tasks spawned with `tokio::spawn` inside the temporary runtime are **cancelled** when
  `block_on` returns (the temporary runtime drops). Long-lived tasks (IMAP polling started
  by `start_polling_inner`) must use `tokio::spawn` inside a context where Tauri's permanent
  runtime is active.
- Plugin stdout readers also get cancelled with the temp runtime — plugin communication
  only works if the plugin sends messages synchronously during setup.

### SMTP
- `ContentType::parse("text/plain")` does NOT work via `Header::parse` trait.
  Use `"text/plain".parse::<ContentType>()` (FromStr) or `ContentType::TEXT_PLAIN`.
- SMTP envelope sender must match authenticated user (relay constraint).

### Frontend
- Tauri v2 has **no `__TAURI__` global** — always import from `@tauri-apps/api` and call `invoke`.
- Frontend uses `@` path alias → `src/` (vite.config.ts).
- Dark mode stored in `localStorage` key `"yse-dark"`, applied via `theme-mode` attribute on `<html>`.

### Plugin system
- Child process JSON-RPC over stdin/stdout. Requests: `send`, `log`.
  Core sends: `message`, `config`, `shutdown`.
- Plugins are **outside** the workspace — compile separately:
  `cd plugins/echo-bot && cargo build`.
- Plugin manager auto-starts enabled plugins in `.setup()`.

### Encryption
- Argon2id → 32B ChaCha20-Poly1305 key. Fixed salt `b"yse-argon2-salt-v1"`.
- 12B random Nonce prefixed to ciphertext (split at index 12 on decrypt).
- Crypto password IS persisted in config JSON (derived into key on load).

### SQLite
- `dirs_next::data_dir()/yse/yse.db`. Created by `YseState::new(db_path)`.
- Config persisted as single JSON row in `config` table key `"config"`.

## CI / Release
- GitHub Actions (`.github/workflows/build.yml`): 4 jobs — desktop (AppImage), android (APK),
  check (core tests + frontend build), release (uploads to GitHub Release `latest` on push to main).
- Flaky step: `android init` has `continue-on-error: true`.
- APK signing uses auto-generated debug keystore.
- Release job runs on push to main after other 3 jobs succeed.

## Git conventions
- Commit messages in Chinese, conventional-commits format: `fix:`, `feat:`, `revert:`.
- AI-generated commits include `Co-authored-by: opencode <deepseek@opencode.com>`.
- Use `kioclient move "file://$path" 'trash:/'` instead of `rm`.
