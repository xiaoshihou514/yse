# Repository Guidelines

## Project Structure & Module Organization

Yse is a Rust workspace for building reactive Qt 6 Widgets applications
without QML. Source lives under `crates/`; tests sit next to their code
(`src` unit tests and `tests/` integration tests per crate); examples and
benchmarks live in each crate's `examples/` directory or under the top-level
`examples/` workspace members.

```text
crates/yse        Facade: re-exports yse-model + yse-ui
crates/yse-model  Pure-Rust reactive runtime (no Qt, no unsafe)
crates/yse-ui     Qt Widgets layer (CXX bridge + C++ shim in src/*.cpp)
crates/yse-tool   Developer CLI (cargo yse new/dev/test/bundle)
examples/taskmgr  Cross-platform task manager built on the `yse` facade
examples/media-converter  Demo app built on yse-ui (uses CXX-Qt build deps)
docs/             Architecture and FFI safety notes
```

`crates/yse-media/` is an empty, untracked scaffolding dir (not a workspace
member, no `Cargo.toml`); don't expect it to build.

Dependency direction: `yse -> yse-ui -> yse-model`. `yse-tool` and the
examples stand apart and link the facade.

## Build, Test, and Development Commands

Install [`just`](https://just.systems/) and use `just --list` for the
shortcuts. `just check` is the full local gate: `fmt --check`, clippy
(`-D warnings`), workspace tests, and `RUSTDOCFLAGS="-D warnings" cargo doc
--workspace --no-deps` — run it before submitting.

```sh
just check                    # the whole gate (fmt, clippy, tests, docs)
cargo build --workspace       # build all crates
cargo test -p yse-model       # fast, Qt-free runtime tests
cargo run -p yse-model --release --example bench_large_table   # benchmark
just example data_browser     # yse-ui example, or workspace app by name
just example taskmgr
```

Qt demos run headless with the offscreen platform plus the smoke mode, which
drives the app end-to-end and prints state before quitting:

```sh
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example settings
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-taskmgr
```

The demo examples (`settings`, `controls`, `data_browser`, `kcalc`, `kalarm`,
`filelight`, `taskmgr`) all work this way.

Windows-only workflow goes through PowerShell helpers — they set a separate
`CARGO_TARGET_DIR`, so Windows and Linux builds never share state:
`just windows setup|diag|build|check`, `just windows-example <name>`,
`just windows-run yse-taskmgr`.

## Coding Style & Naming Conventions

- Format with `just fmt`; lint with `just lint` (clippy
  `--workspace --all-targets -- -D warnings`).
- Rust: 4-space indent, `snake_case` items, `UpperCamelCase` types, `rustfmt`
  defaults. C++ shims follow the same 2-space indentation used in
  `crates/yse-ui/src/widgets.cpp`.
- New public API needs doc comments; `just check` fails on doc warnings.
- `yse-model` must stay free of `unsafe` (`#![forbid(unsafe_code)]`); FFI
  `unsafe` belongs only in `yse-ui`. See [docs/safety.md](docs/safety.md) for
  the FFI invariants (pointer lifetimes, destroyed hook, panic containment).

## Testing Guidelines

Run `cargo test --workspace`. Tests are plain `#[test]` functions named
`snake_case` describing the behavior (e.g., `diamond_graph_is_glitch_free`).
yse-ui tests set `QT_QPA_PLATFORM=offscreen` inside each binary; dialog tests
(`message_box.rs`, `file_dialog.rs`) live one per test binary because Qt
dialogs are sensitive to thread affinity. Keep the local gate green before
submitting: `just check`.

## Commit & Pull Request Guidelines

- Commit messages are concise Chinese sentences describing the change (see
  `git log`); keep the first line under 80 characters, and append the
  `Co-authored-by: opencode <deepseek@opencode.com>` trailer.
- No CI workflows are committed (GitHub Actions quota); reviewers run the
  local gate instead.
- Pull requests: describe what changed and why, note any manual verification
  performed (e.g., headless smoke output), and keep changes scoped to one
  concern.

## Architecture Overview

Reactive semantics, the C++ shim pattern, and lifecycle guarantees are
documented in [docs/architecture.md](docs/architecture.md). Read it before
touching the graph core or the widget bridge.
