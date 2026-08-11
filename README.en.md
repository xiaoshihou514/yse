# Yse

Rust-first framework for building serious, cross-platform Qt 6 Widgets
applications without QML. Uses CXX-Qt as the interoperability backend and
borrows its reactive programming model from Airstream and Laminar.

Yse is for data-rich native desktop applications that want Qt's mature
widgets and Linux integration without writing QML or manually managing signal
connection lifetimes.

> **Pre-release:** the API is still changing and the crates are not published.
> Use a local checkout while the clean-machine and packaging paths are hardened.

## Five-minute local start

You only need Rust and a C++ toolchain. `gansi` downloads and manages Qt 6
itself.

Install the C++ toolchain for your platform:

```sh
# Fedora
sudo dnf install gcc-c++ cmake ninja-build pkgconf-pkg-config

# Ubuntu / Debian
sudo apt install g++ cmake ninja-build pkg-config

# openSUSE Tumbleweed
sudo zypper install gcc-c++ cmake ninja lld pkgconf

# Windows (Visual Studio Build Tools 2022 with the "Desktop development with C++" workload)
#   + CMake and Ninja: scoop install cmake ninja
```

Then let `gansi` fetch Qt:

Create and run an application against this checkout:

```sh
cargo install --path crates/gansi
gansi doctor
cd /tmp
gansi create --local /path/to/yse hello-yse
cd hello-yse
gansi run
```

`gansi setup` (run automatically by the commands above when needed) downloads
prebuilt Qt 6 binaries into the gansi data directory — no `qt6-*-devel`
packages and no manual Qt installer.

The generated application owns reactive Rust state, derives its label text,
and releases widget bindings with the Qt object tree. Its minimal `build.rs`
retains CXX-Qt dependency initializers; no application-owned C++ or QML is
generated.

## Supported environments

| Environment | Status |
|---|---|
| Fedora, Qt 6, GCC, Wayland/offscreen | Primary development environment |
| Ubuntu, Qt 6, GCC | Supported; clean-machine validation in progress |
| KDE Plasma and GNOME | Intended Tier 1 desktops; full UX matrix in progress |
| Windows | Build helpers and task-manager backend available |
| macOS | Experimental; does not block the first release |

Run the full display-free Fedora acceptance path without sudo:

```sh
CCACHE_DISABLE=1 just release-check
```

This does not publish or upload anything. It runs the workspace, MSRV,
dependency-license, standalone crate-package, generated-project, example, and
Valgrind gates. The Linux smoke gate builds and executes the default release bundle; use
`YSE_SMOKE_BUNDLE_PROFILE=debug just linux-smoke` for quicker packaging work.
When a live Wayland session is available, `just examples-wayland-smoke` runs
the same eight application flows against the native display plugin.

See [docs/validation.md](docs/validation.md) for the exact verified environment
and the platform scenarios that remain unverified.

See [docs/roadmap-status.md](docs/roadmap-status.md) for a requirement-by-requirement
mapping from the Phase 0–4 plan to implemented and verified evidence.

See [agent_docs/PLAN.md](agent_docs/PLAN.md) for the full roadmap.

Architecture notes are in [docs/architecture.md](docs/architecture.md) and the
release process in [RELEASE.md](RELEASE.md). Qt and CXX-Qt have separate
distribution obligations summarized in [docs/licensing.md](docs/licensing.md).

## Current state

The implementation described by Phase 0 (CXX-Qt feasibility spike) through
Phase 4 (`gansi`) is present. Fedora-local functional and packaging gates pass,
but the roadmap's clean-host and multi-platform exit criteria are not yet all
verified; see [docs/validation.md](docs/validation.md). The
[`yse`](crates/yse) facade re-exports the public API of both layers, so
applications can `use yse::*` for the whole stack.

[`yse-model`](crates/yse-model) is a pure-Rust reactive runtime (no Qt, no
`unsafe`) with transactional, glitch-free propagation, mandatory subscription
ownership, the initial operator set, and a cancellable async task API
(`spawn_task`) whose results are delivered onto the graph's thread through a
[`Scheduler`](https://docs.rs/yse-model) (see the `QueueScheduler` for
deterministic tests). It also provides `ListModel<T>`, an incremental list
with a current snapshot and a stream of structural changes (`Insert`/`Remove`/
`Update`/`Reset`), plus in-place `sort_by`/`retain` that emit incremental
changes, and an `UndoStack` with a `Command` trait whose `can_undo`/`can_redo`
state is reactive. It also ships `Diagnostics` (a stream of transactional
graph events: nodes processed, observers fired, deferred writes, cycle
rejections — useful for explaining why a widget changed) and a leveled
`Logger` with an observable record stream. Run its tests with:

```sh
cargo test -p yse-model
```

[`yse-ui`](crates/yse-ui) binds `yse-model` to a retained Qt Widgets tree
through a small C++ shim: window, row/column layouts, label, button, line
edit, checkbox, combo box, spin box, slider, progress bar, date/time edits,
and grid layouts (with generic `add` for nesting), plus actions, menus, menu
bars, and toolbars with shortcuts. It has signal-to-property bindings, two-way
line-edit bindings, and per-component owners released automatically when the
Qt object is destroyed. A `QtGuiScheduler` runs `yse_model::spawn_task`
results on the Qt event loop, and `yse_model`'s time operators (`debounce`,
`throttle`, `delay`) are driven by a `QtTimer` in applications and a
`ManualTimer` in tests.
`StringListModel` drives a `QListView` through a C++ `QAbstractListModel`
adapter, applying changes incrementally with `beginInsertRows`/`beginRemoveRows`
so the view never resets for ordinary edits; `ListView::selection_changed()`
exposes `QItemSelectionModel` changes as a stream of row indices. The settings
demo wires Edit-menu Undo/Redo actions to an `UndoStack` via `bind_enabled`.
`StringTableModel` does the same for a multi-column `QTableView`
(`QAbstractTableModel` with headers, incremental row insert/remove/update).
Standard dialogs are available as reactive wrappers: `MessageBox`
(`Ok`/`OkCancel`/`YesNo`, result stream) and `FileDialog` (open-file, optional
path result), both non-modal and headless-testable.
`Settings` provides QSettings-backed key/value persistence with string values.

For large-table performance, a rough benchmark is included:

```sh
cargo run -p yse-model --release --example bench_large_table
```

On this machine a 100k-row load takes ~11 ms, 10k per-row incremental updates
~6 ms, and an in-place sort ~9 ms — all without rebuilding the widget tree.

The Phase 3 demonstration, [`data_browser`](crates/yse-ui/examples/data_browser.rs),
assembles the stack: it asynchronously loads records (with cancellation and
error states), filters and sorts an incremental table, edits rows through a
form with undo/redo, persists settings, and exposes menu shortcuts. Run it
headless with:

```sh
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example data_browser
```

It prints `status`, visible row count, active filter, and the first visible
row after driving the whole flow.

## Developer toolchain (`gansi`)

[`gansi`](crates/gansi) is the Phase 4 developer CLI. Install it with:

```sh
cargo install --path crates/gansi
```

Then:

```sh
gansi create hello        # inside this checkout, auto-detect the local Yse facade
gansi create --local /path/to/yse hello-facade  # elsewhere, select the checkout explicitly
cd hello
gansi run                 # build and run
gansi test                # run tests
gansi analyze             # Clippy, all targets, warnings denied
gansi format --check      # verify rustfmt output
gansi upgrade -- --offline # refresh Cargo.lock without network access
gansi add serde -- --features derive # add a Cargo dependency
gansi build               # release build + platform bundle (windeployqt/macdeployqt when present)
```

Linux bundles include a launcher, application binary, required Qt libraries,
selected desktop plugins, a Qt runtime inventory, and discoverable Qt license
texts. Generated freedesktop and AppStream metadata plus a scalable icon are included under
`share/` for package integration. `GLIBC_REQUIREMENTS.tsv` records the newest
glibc symbol required by each bundled ELF object. Bundles intentionally keep glibc, graphics
drivers, and non-Qt system libraries as target-system dependencies, listing
their resolved build-host paths in `SYSTEM_RUNTIME.tsv`. A `.tar.gz` artifact
preserves the complete relocatable directory for transfer and testing, with a
SHA-256 digest in `dist/SHA256SUMS`.

Generated projects pin the Qt version, compiler family, and target
architecture detected at creation in `gansi.toml`; `run`, `test`, and `build`
reject mismatches before invoking Cargo. They also include a `RELEASE.md` guide that lists what
stays application-specific (code signing, notarization, store submissions,
native dependencies).
Qt SDK archives downloaded by `gansi setup` are verified against per-archive
checksum sidecars before extraction. SHA-256 is preferred, with SHA-1 fallback
for Qt repository layouts that do not publish SHA-256 sidecars.
The `--local` variant depends on the `yse` facade crate from your checkout,
so generated apps exercise the full framework without a C++ shim.
Until the crates are published, `gansi create` refuses registry resolution when
it cannot discover a checkout. Failed dependency setup is staged separately and
does not leave a partial project directory behind.
Run its lifecycle tests and the headless settings-form demo with:

```sh
cargo test -p yse-ui
QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui --example settings
```

The settings demo drives the form end-to-end in smoke mode: it types a name
through the two-way binding, clicks submit (enabled by derived state), loads
state from a background task delivered on the GUI thread, and triggers a File
menu action, printing the resulting greeting, status, and load state before
quitting.

The [`controls`](crates/yse-ui/examples/controls.rs) demo shows the value
widgets in a declarative tree: a combo box, spin box, slider, and progress
bar wired together with signal bindings and value-change handlers. Run it
headless with `QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-ui
--example controls`. FFI safety invariants — pointer lifetimes, the destroyed
hook, panic containment at the C++ boundary, and the widget-extension pattern
— are documented in [docs/safety.md](docs/safety.md).

[`yse-taskmgr`](examples/taskmgr) is a cross-platform system task manager
(Windows / Linux) built entirely in the Laminar style: all state lives in
`Var`s, the UI binds to derived signals (`bind_rows`, `bind_series`,
`bind_visible`, ...), and event handlers only mutate state. System data is
collected by platform-specific backends selected with `cfg`: `/proc` and
`/sys` on Linux, `windows-sys` on Windows. It follows the system color scheme
(light/dark), shows real per-process icons (shell icons on Windows, PATH
lookups on Linux), and is styled with a modern Fusion + stylesheet look. Run
it headless with
`QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-taskmgr` (or
`just windows-run yse-taskmgr` on the Windows host).

## Development environment

- Rust 1.89 or newer (edition 2024)
- A C++17 compiler and CMake/Ninja (see the platform commands above)
- Qt 6 is downloaded and managed by `gansi setup`; do not install
  `qt6-base-dev` / `qt6-qtbase-devel` yourself

## Build and run

```sh
just --list                  # list development shortcuts
just check                   # format check, lint, tests, and docs
cargo build
```

Install [`just`](https://just.systems/) to use the shortcuts; the underlying
Cargo commands remain usable directly.

## CI

Workflows are intentionally omitted to conserve GitHub Actions quota. The
workspace is validated locally with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```
