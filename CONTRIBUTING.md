# Contributing to Yse

Yse is currently in pre-release hardening. Small, focused changes with tests
are preferred over broad API expansion.

## Development

Install Rust, a C++17 compiler, CMake, Ninja, pkg-config, and Qt 6 Widgets.
Run `cargo run -p gansi -- doctor --verbose` to inspect the environment.

Before submitting a change:

```sh
CCACHE_DISABLE=1 just check
just linux-smoke
```

The Linux smoke test is display-free, needs no sudo, works offline once Cargo
dependencies are cached, and exercises generated-project creation, tests,
execution, and bundling.

## Design constraints

- Keep `yse-model` Qt-free and free of `unsafe`.
- Preserve GUI-thread delivery and Qt-object-owned subscription lifetimes.
- Final CXX-Qt binaries require the minimal initializer `build.rs` used by the
  generated project template.
- Treat Qt, CXX-Qt, CXX, Cargo dependencies, and bundled native libraries as
  separate licensing inputs; see `docs/licensing.md`.
- Add public API documentation and behavior-focused tests.

## Changes

Record user-visible behavior and compatibility changes under `Unreleased` in
`CHANGELOG.md`. The pre-release API may break, but changes should remain
intentional and documented.
