# Yse architecture

Yse is a Rust-first framework for serious, cross-platform Qt 6 Widgets
applications without QML. CXX-Qt owns the Rust <-> Qt interoperability; Yse
owns reactive state, widget composition, and developer tooling.

## Workspace layout

```text
crates/yse        Convenience facade: re-exports yse-model + yse-ui
crates/yse-model  Pure-Rust reactive runtime (no Qt, no unsafe)
crates/yse-ui     Reactive Qt Widgets layer (CXX bridge + C++ shim)
crates/gansi      Developer toolchain (`gansi create/run/test/build`)
```

Dependency direction: `yse -> yse-ui -> yse-model`. `gansi`, examples, and
the example crates stand apart. `yse-model` is independent of Qt and
deterministic; `yse-ui` is a retained object tree (no virtual DOM).

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
- A transaction body that panics still settles the writes it already made,
  clears transaction bookkeeping, and then resumes the original panic. A
  panicking propagation pass discards its queued stream inputs and resets all
  scheduled markers, so a later transaction is never silently skipped.
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

Widget composition is a small hand-written C++ Widgets shim
(`src/widgets.cpp` + `src/model.cpp`) bridged with cxx. Qt-facing reactive
state is implemented as generated CXX-Qt `QObject`s under `src/qt_object.rs`;
the generated properties and signals are the boundary between Rust state and
Qt notification. The shim owns widget construction, while Rust owns state and
lifecycle.

- The Rust component tree mirrors Qt parentage (strong parent-to-child,
  weak child-to-parent). It retains callback state, models, selections,
  actions, and non-modal dialogs for exactly the native lifetime that can call
  them. Every component owns an `Owner` tied to the Qt object lifetime: a
  `destroyed` hook clears bindings, and wrapper drop disconnects only its own
  saved Qt connections, so both teardown orders are safe without disturbing
  embedder-installed connections.
- Bindings: signal-to-property (`bind_text`, `bind_enabled`, ...), widget
  signals to `EventStream` (`clicked()`, `text_changed()`, `toggled()`,
  `value_changed()`, `header_clicked()`, `selection_changed()`, `triggered()`),
  and two-way bindings (`bind_text_two_way`, `bind_value_two_way`,
  `bind_checked`) with feedback-loop guards. Tables, button labels, chart
  series, and widget visibility can all be bound to signals
  (`bind_rows`, `bind_text`, `bind_series`, `bind_visible`), so applications
  can be written Laminar-style: state lives in `Var`s, the UI is a pure
  function of derived signals, and event handlers only mutate state. Labels
  use the generated `TextState` QObject, so a normal Qt property signals
  update the widgets rather than one-off Rust-to-C++ callbacks. `TextState`
  backs labels and line edits, and `ToggleState` keeps a checkbox's `checked`
  property synchronized in both directions. `ActionState` owns QAction text
  and enabled state used by menus and toolbars. Every QWidget wrapper has a
  generated `WidgetState` for its shared enabled, visibility, and title
  properties.
- `StringListModel`/`StringTableModel` drive `QListView`/`QTableView` through
  C++ `QAbstractItemModel` mirrors using `beginInsertRows`/`beginRemoveRows`/
  `dataChanged`, so views update incrementally.
- `TabWidget` pages, a painted `LineChart` for time-series data, and
  date/time editors complete the initial desktop surface.
- Actions/menus/toolbars, `MessageBox`/`FileDialog`, and `Settings`
  (QSettings-backed persistence) complete the desktop primitives.

## Example-local native code

Optional native integrations used by one demonstration stay with that example.
For instance, `examples/media-converter/` owns its FFmpeg Rust facade, C++ bridge
sources in `cpp/`, and build script; Qt Widgets and FFmpeg linkage do not leak
into `yse-ui`.

## Toolchain (`gansi`)

`gansi create` generates a buildable project template and pinned
configuration (`gansi.toml`). Inside a yse checkout it defaults to the facade
template (`use yse::*`), and outside that context it falls back to the
standalone CXX-Qt template.
`run`/`test`/`build` wrap Cargo and Qt deployment tooling.

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
- The crates are not yet published; outside a local yse checkout,
  `gansi create` falls back to the standalone CXX-Qt shape rather than
  depending on the facade.
