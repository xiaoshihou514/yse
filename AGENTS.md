# YSE (盐水鹅)

Email-mediated chat + plugin automation system. Desktop app via Tauri 2, Android via Tauri 2 mobile.

## Repo structure

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure business logic (no platform deps)
├── desktop/    # yse-desktop: Tauri 2 app, bridges core ↔ frontend
├── mobile/     # yse-mobile: Tauri 2 mobile stub (Android)
└── frontend/   # Vue 3 + Pinia + TDesign + vue-router, built to dist/
```

## Key commands

```sh
cargo check -p yse-core          # Fast: check core only
cargo test -p yse-core           # 9 tests
cargo check                      # Full workspace
cargo clippy -- -D warnings      # Lint
cargo fmt                        # Format Rust

just dev                          # cargo tauri dev (Vite + Tauri together)
just fe-dev                       # Vite dev server at :1420 (from frontend/)
just fe-build                     # npm run build (vue-tsc --noEmit && vite build)
just fe-typecheck                 # Frontend TS type check only
just check-all                    # fe-typecheck + cargo check
just build-appimage               # AppImage bundle
just build-deb                    # deb bundle

npm run dev                       # Vite only (from frontend/)
npm run build                     # vue-tsc --noEmit && vite build
```

## Architecture notables

- **Encryption**: Argon2id → 32B ChaCha20-Poly1305 key. 12B random Nonce prefixed to ciphertext. Fixed salt `b"yse-argon2-salt-v1"` in `core/src/crypto.rs`.
- **IMAP**: Sync `imap` crate (v2, native-tls). 10s polling, no IDLE. Session not `Send` — do not hold across `.await` in Tauri commands. First poll fetches ALL emails (`last_uid` starts `None`).
- **SMTP**: `lettre` 0.11 (tokio1-rustls-tls, smtp-transport, builder). Sends multipart/mixed with base64 Content-Transfer-Encoding. SMTP envelope sender must match authenticated user (relay constraint).
- **Disguise**: Random sender from hot domain pool (gmail/outlook/yahoo/proton/icloud/qq/163). QQ/163 get digit usernames; other domains get `name.surname[digits]` style. Default config uses QQ IMAP/SMTP ports.
- **Plugin system**: Child process via stdin/stdout JSON-RPC. Plugin requests: `send`, `log`. Core notifications: `message`, `config`, `shutdown`.
- **Tauri commands** (15): `send_message`, `get_messages`, `get_config`, `save_config`, `start_polling`, `stop_polling`, `list_plugins`, `add_plugin`, `remove_plugin`, `toggle_plugin`, `start_plugin`, `stop_plugin`, `list_running_plugins`, `get_logs`, `test_email`. State managed in `YseState` (commands.rs), IMAP poller emits `new-message` events via `app.emit`.
- **SQLite DB**: `dirs_next::data_dir()/yse/yse.db`. Created by `YseState::new(db_path)`. Config persisted as JSON in `config` table key `"config"`. Crypto password IS persisted in config (derived into key on load).
- **Tauri config**: `desktop/tauri.conf.json`. Frontend dist at `../frontend/dist`. Dev URL `http://localhost:1420`.

## Requirements

- **Linux Tauri build**: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `libjavascriptcoregtk-4.1-dev`, `libsoup-3.0-dev`, `patchelf`
- **Rust**: edition 2021
- **Node**: for frontend (Vite, vue-tsc)
- **Android**: Java 17, Android SDK, NDK (OpenSSL vendored in mobile crate)

## Gotchas

- `imap::Session` is not `Send` → do not hold across `.await`. Wrap sync calls tightly.
- `imap` v2 uses `native-tls` (needs OpenSSL runtime). `imap-proto` has a future-compat warning (third party, ignore).
- SMTP `ContentType::parse("text/plain")` does NOT work via `Header::parse` trait path. Use `"text/plain".parse::<ContentType>()` (FromStr) or constants (`ContentType::TEXT_PLAIN`).
- Tauri v2 app section has no `title` field; title lives on each window entry.
- Tauri `.setup()` hook runs **before** Tokio runtime is available. Use `tokio::runtime::Runtime::new()` + `block_on` or `tauri::async_runtime::spawn` for async work there.
- `@tauri-apps/api` in Tauri v2 has no `__TAURI__` global; always import and call `invoke` directly.
- MIME body parsing strips trailing CRLF from encrypted body (`core/src/email/parser.rs:25`).
- Plugins are standalone executables outside the workspace; compile separately via `cd plugins/echo-bot && cargo build`.
- Frontend uses `@` path alias → `src/` (configured in `vite.config.ts`).
- Dark mode stored in `localStorage` key `"yse-dark"`; toggled by setting `theme-mode` attribute on `<html>`.
