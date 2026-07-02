# YSE (盐水鹅)

Email-mediated chat + plugin automation system. Desktop app via Tauri 2.

## Repo structure

```
Cargo workspace (resolver = "2")
├── core/       # yse-core: pure business logic (no platform deps)
├── desktop/    # yse-desktop: Tauri 2 app, bridges core ↔ frontend
├── mobile/     # yse-mobile: Tauri 2 mobile stub
└── frontend/   # Vue 3 + TDesign, built to ../frontend/dist
```

## Key commands

```sh
npm run dev          # Vite dev server at :1420 (from frontend/)
cargo tauri dev      # Dev mode (starts Vite + Tauri together)
cargo check -p yse-core   # Fast: check core only
cargo test -p yse-core    # 9 tests
cargo check               # Full workspace
npm run build             # frontend/: vue-tsc --noEmit && vite build
```

## Architecture notables

- **Encryption**: Argon2id → 32B ChaCha20-Poly1305 key. 12B random Nonce prefixed to ciphertext. Fixed salt in `crypto.rs`.
- **IMAP**: Uses sync `imap` crate (v2, native-tls). No IDLE support (falls back to 10s polling). Session not `Send` across async boundaries.
- **SMTP**: `lettre` 0.11 (tokio1-rustls-tls, smtp-transport, builder). Sends multipart/mixed with base64 Content-Transfer-Encoding.
- **Disguise**: Random sender from hot domain pool (gmail/outlook/qq/163/yahoo/proton/icloud). QQ/163 get digit usernames; other domains get `name.surname[digits]` style.
- **Plugin system**: Child process via stdin/stdout JSON-RPC. Commands: `send`, `log`, `notify`. Notifications: `message`, `config`, `shutdown`.
- **Frontend ↔ Core**: 14 Tauri commands via `@tauri-apps/api` invoke. State managed in Pinia store (`useYseStore`). IMAP poller emits `new-message` events.
- **SQLite DB**: `dirs_next::data_dir()/yse/yse.db` (~/.local/share/yse/ on Linux). Created by `YseState::new(db_path)`.
- **Tauri config**: `desktop/tauri.conf.json`. Frontend dist at `../frontend/dist`. Dev URL `http://localhost:1420`.

## Requirements

- **Linux Tauri build**: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev` (and deps)
- **Rust**: edition 2021
- **Node**: for frontend build (vite, vue-tsc)

## Gotchas

- `imap::Session` is not `Send` → do not hold across `.await` in Tauri commands. Wrap sync calls tightly.
- `imap` v2 uses `native-tls` (needs openssl runtime). `imap-proto` has a future-compat warning (third party, ignore).
- SMTP `ContentType::parse("text/plain")` does NOT work via `Header::parse` trait path. Use `"text/plain".parse::<ContentType>()` (FromStr) or constants (`ContentType::TEXT_PLAIN`).
- Tauri v2 app section has no `title` field; title lives on each window entry.
- Tauri `.setup()` hook runs **before** the Tokio runtime is available. Use `tokio::runtime::Runtime::new()` (+ `block_on`) or `tauri::async_runtime::spawn` for async work there.
- `@tauri-apps/api` in Tauri v2 has no `__TAURI__` global; always import and call `invoke` directly.
- Config persisted as JSON in SQLite `config` table key `"config"`. Crypto password NOT persisted (derived on each login).
- MIME body parsing strips trailing CRLF from encrypted body.
