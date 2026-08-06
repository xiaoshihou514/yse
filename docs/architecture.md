# Yse architecture

Yse is a Rust-first framework for serious, cross-platform Qt 6 Widgets
applications without QML. CXX-Qt owns the Rust <-> Qt interoperability; Yse
owns reactive state, widget composition, and developer tooling.

## Workspace layout

```text
crates/yse        Convenience facade: re-exports yse-model + yse-ui
crates/yse-model  Pure-Rust reactive runtime (no Qt, no unsafe)
crates/yse-ui     Reactive Qt Widgets layer (CXX bridge + C++ shim)
crates/yse-tool   Developer toolchain (`cargo yse new/dev/test/bundle`)
spike             Phase 0 feasibility spike (CXX-Qt Widgets without QML)
```

Dependency direction: `yse -> yse-ui -> yse-model`, with `yse-tool` and the
spike standing apart. `yse-model` is independent of Qt and deterministic;
`yse-ui` is a retained object tree (no virtual DOM).

## Reactive model (`yse-model`)

The graph is synchronous, single-threaded, and reference-counted
(`Rc<RefCell<...>>` behind safe APIs; `#![forbid(unsafe_code)]`).

- `EventStream<T>` carries discrete events; `Signal<T>` always holds a value;
  `Var<T>`/`Sink<T>` are writable sources.
- Transactions are glitch-free: derived nodes recompute in topological rank
  order, and observer callbacks are delivered after the pass completes, so a
  diamond never observes inconsistent intermediate state.
- Multiple writes in one `transaction(...)` block notify observers once, with
  the final value. Writes performed during propagation are queued and applied
  in FIFO order in a follow-up pass.
- Every observation returns a `Subscription`; dropping it (directly or through
  an `Owner`) unsubscribes. Signals always keep current values; streams start
  with their first observer and stop with their last.
- `spawn_task(scheduler, f)` runs `f` on a worker thread with a
  `CancellationToken` and delivers results onto the graph's thread through a
  `Scheduler` (`QueueScheduler` for tests, `QtGuiScheduler` on the Qt event
  loop). Cancelled or dropped tasks never deliver.
- `ListModel<T>` emits precise structural changes (`Insert`/`Remove`/
  `Update`/`Reset`); `UndoStack` + `Command` provide reversible edits with
  reactive `can_undo`/`can_redo`; `Diagnostics` reports every transaction's
  processed nodes, observer fires, deferred writes, and cycle rejections.

## Widget layer (`yse-ui`)

Each widget family is a Rust wrapper around a hand-written C++ shim
(`src/widgets.cpp` + `src/model.cpp`), bridged with cxx. The shim owns raw
Qt objects; Rust owns state and lifecycle.

- Every component owns an `Owner` tied to the Qt object lifetime: a
  `destroyed` hook clears bindings, and wrapper drop performs targeted signal
  disconnects, so both teardown orders (wrapper first or widget first) are
  safe.
- Bindings: signal-to-property (`bind_text`, `bind_enabled`, ...), widget
  signals to `EventStream` (`clicked()`, `text_changed()`, `toggled()`,
  `selection_changed()`, `triggered()`), and two-way line-edit bindings with a
  feedback-loop guard.
- `StringListModel`/`StringTableModel` drive `QListView`/`QTableView` through
  C++ `QAbstractItemModel` mirrors using `beginInsertRows`/`beginRemoveRows`/
  `dataChanged`, so views update incrementally.
- Actions/menus/toolbars, `MessageBox`/`FileDialog`, and `Settings`
  (QSettings-backed persistence) complete the desktop primitives.

## Toolchain (`yse-tool`)

`cargo yse new` generates a buildable CXX-Qt project with pinned
configuration (`yse.toml`), an icon, and a release guide.
`dev`/`test`/`bundle` wrap Cargo and Qt deployment tooling.

## Testing strategy

- `yse-model`: semantic unit/integration tests cover glitch-free diamonds,
  transaction batching, re-entrant writes, ownership, restart re-sync, panic
  safety, cycle rejection, async cancellation/FIFO delivery, list changes,
  undo/redo, diagnostics, and logger filtering.
- `yse-ui`: headless tests (offscreen platform) cover lifecycle release,
  incremental list/table updates, selection streams, dialogs, and settings
  round-trips. Dialog tests live one-per-binary because Qt dialogs are
  sensitive to test-thread affinity.
- Examples: `settings` and `data_browser` demos have deterministic `YSE_SMOKE`
  headless runs; `bench_large_table` reports incremental update cost.
- Validation: the local gate (`cargo test --workspace`, clippy with
  `-D warnings`, `cargo fmt --check`, `cargo doc`) stands in for CI, which is
  intentionally not committed to conserve Actions quota.

## Known limitations

- The reactive graph is single-threaded by design; background work must return
  through a scheduler.
- Windows/macOS builds are only verified from this development machine;
  platform CI can be added later when quota allows.
- The crates are not yet published; `cargo yse new` templates therefore use
  the standalone CXX-Qt shape rather than depending on the facade.
