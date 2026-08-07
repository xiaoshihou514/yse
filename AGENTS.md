# Repository Guidelines

## Project Structure & Module Organization

Yse is a Rust workspace for building reactive Qt 6 Widgets applications
without QML. Source lives under `crates/`; tests sit next to their code
(`src` unit tests and `tests/` integration tests per crate); examples and
benchmarks live in each crate's `examples/` directory.

```text
crates/yse        Facade: re-exports yse-model + yse-ui
crates/yse-model  Pure-Rust reactive runtime (no Qt, no unsafe)
crates/yse-ui     Qt Widgets layer (cxx bridge + C++ shim in src/*.cpp)
crates/yse-tool   Developer CLI (`gansi` new/setup/dev/test/bundle)
spike             Phase 0 CXX-Qt feasibility spike
docs/             Architecture notes
```

Dependency direction: `yse -> yse-ui -> yse-model`.

## Build, Test, and Development Commands

```sh
cargo build                       # build all workspace crates
cargo test --workspace            # run every unit and integration test
cargo run -p yse-spike            # run the Phase 0 spike
cargo run -p yse-ui --example data_browser   # run the data-browser demo
cargo run -p yse-model --release --example bench_large_table  # benchmark
```

Headless runs need Qt's offscreen platform:
`QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example settings`.

## Coding Style & Naming Conventions

- Format with `cargo fmt`; lint with `cargo clippy --workspace --all-targets -- -D warnings`.
- Rust: 4-space indent, `snake_case` items, `UpperCamelCase` types, `rustfmt`
  defaults. C++ shims follow the same 2-space indentation used in
  `crates/yse-ui/src/widgets.cpp`.
- New public API needs doc comments; the workspace builds with
  `RUSTDOCFLAGS="-D warnings" cargo doc`.
- `yse-model` must stay free of `unsafe` (`#![forbid(unsafe_code)]`); FFI
  `unsafe` belongs only in `yse-ui`.

## Testing Guidelines

Run `cargo test --workspace`. Tests are plain `#[test]` functions named
`snake_case` describing the behavior (e.g., `diamond_graph_is_glitch_free`).
Headless Qt tests set `QT_QPA_PLATFORM=offscreen`; dialog tests live one per
test binary because Qt dialogs are sensitive to thread affinity. Keep the
local gate green before submitting: tests, clippy, `cargo fmt --check`.

## Commit & Pull Request Guidelines

- Commit messages are concise Chinese sentences describing the change (see
  `git log`); keep the first line under 80 characters.
- No CI workflows are committed (GitHub Actions quota); reviewers run the
  local gate instead.
- Pull requests: describe what changed and why, note any manual verification
  performed (e.g., headless smoke output), and keep changes scoped to one
  concern.

## Architecture Overview

Reactive semantics, the C++ shim pattern, and lifecycle guarantees are
documented in [docs/architecture.md](docs/architecture.md). Read it before
touching the graph core or the widget bridge.
