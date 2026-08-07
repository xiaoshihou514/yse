# Yse

Rust-first framework for building serious, cross-platform Qt 6 Widgets
applications without QML. Uses CXX-Qt as the interoperability backend and
borrows its reactive programming model from Airstream and Laminar.

See [agent_docs/PLAN.md](agent_docs/PLAN.md) for the full roadmap.

Architecture notes are in [docs/architecture.md](docs/architecture.md) and the
release process in [RELEASE.md](RELEASE.md).

## Current state

Phase 0 (CXX-Qt feasibility spike) through Phase 4 (`yse-tool`) are
implemented. The [`yse`](crates/yse) facade re-exports the public API of both
layers, so applications can `use yse::*` for the whole stack.

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

## Developer toolchain (`cargo yse`)

[`yse-tool`](crates/yse-tool) is the Phase 4 developer CLI. Install it with:

```sh
cargo install --path crates/yse-tool
```

Then:

```sh
cargo yse new hello        # generate a project (buildable CXX-Qt template)
cargo yse new --local /path/to/yse hello-facade  # generate against a local Yse checkout
cd hello
cargo yse dev              # build and run
cargo yse test             # run tests
cargo yse bundle           # release build + platform bundle (windeployqt/macdeployqt when present)
```

Generated projects pin the Qt version, compiler family, and target
architecture in `yse.toml`, include a three-platform CI workflow, an icon
placeholder, and a `RELEASE.md` guide that lists what stays application-specific
(code signing, notarization, store submissions, native dependencies).
The `--local` variant depends on the `yse` facade crate from your checkout,
so generated apps exercise the full framework without a C++ shim.
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
`/sys` on Linux, `windows-sys` on Windows. Run it headless with
`QT_QPA_PLATFORM=offscreen YSE_SMOKE=1 cargo run -p yse-taskmgr` (or
`just windows-run yse-taskmgr` on the Windows host).

## Development environment

- Rust toolchain (edition 2024)
- C++17 compiler and CMake
- Qt 6 with the Widgets module:
  `sudo apt install -y qt6-base-dev ninja-build libgl1-mesa-dev`

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
