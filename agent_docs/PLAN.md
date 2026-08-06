# Yse plan: Phase 0 through Phase 4

## Product direction

Yse is a Rust-first framework for building serious, cross-platform Qt 6 Widgets
applications without QML. It uses CXX-Qt as the interoperability backend and
borrows its reactive programming model from Airstream and Laminar.

The public promise is:

> Build and ship reactive Qt desktop applications from Rust, with lifecycle-safe
> signals and a coherent Cargo-first developer experience.

Yse is not a Qt binding replacement. CXX-Qt owns Rust <-> Qt interoperability;
Yse owns reactive state, widget composition, application architecture, and
developer tooling.

## Initial workspace

```text
yse-model  Pure Rust reactive runtime, inspired by Airstream
yse-ui     Reactive Qt Widgets layer, inspired by Laminar
yse        Convenience facade that re-exports the public application API
```

Future packages, deliberately out of scope until later:

```text
yse-tool   Project creation, development, test, and packaging commands
yse-kde    Optional KDE Framework integrations
```

Dependency direction:

```text
yse -> yse-ui -> yse-model
              -> CXX-Qt -> Qt 6 Widgets
```

`yse-model` must remain independent of Qt and usable in deterministic unit
tests. `yse-ui` must not implement a virtual DOM: Qt Widgets is already a
retained object tree.

## Core design principles

- Model events and state separately: `EventStream<T>` has no current value;
  `Signal<T>` does.
- Make graph propagation transactional and glitch-free.
- Require an `Owner` for every long-lived observation so subscriptions are
  destroyed with their component.
- Keep UI signals on the Qt GUI thread. The reactive graph is local by default,
  not `Send` or `Sync`.
- Make background work explicit: it must return through a UI-thread dispatcher
  as a new transaction.
- Preserve an escape hatch to the underlying Qt object when Yse has not yet
  wrapped a capability.
- Prefer a small, reliable initial widget surface over broad incomplete Qt API
  coverage.
- Treat packaging as a first-class feature, but do not promise cross-compiling
  from one host. Build reproducibly on each target platform instead.

## Phase 0: CXX-Qt Widgets feasibility spike

### Objective

Prove that CXX-Qt can support the no-QML Qt Widgets path with a small,
maintainable generated C++ shim.

### Deliverables

- A Rust application that starts `QApplication` and opens a `QWidget` window.
- A CXX-Qt-backed wrapper for a label, button, and vertical layout.
- Button clicks delivered to Rust and a Rust-originated label update.
- A reliable hook for `QObject::destroyed`.
- A background task whose result is dispatched safely onto the GUI thread.
- A minimal build on Linux, Windows, and macOS CI runners.

### Exit criteria

- No QML appears in the application or generated user-facing API.
- The C++ shim is small, reviewed, and has a clear expansion pattern.
- Widget destruction cannot leave a callable Rust closure behind.
- All three platforms compile and run a smoke test.

### Decision gate

If CXX-Qt requires a large bespoke shim for ordinary widgets, pause before
building Yse's public abstractions and reassess the bridge or upstream
contribution strategy.

## Phase 1: `yse-model` minimum viable reactive runtime

### Objective

Specify and implement the reactive semantics before any UI API depends on them.

### Public types

- `EventStream<T>`
- `Signal<T>`
- `Var<T>`
- `Observer<T>` or `Sink<T>`
- `Subscription`
- `Owner`
- `Scheduler` / UI dispatcher boundary

### Initial operators

- `map`, `filter`, `filter_map`, `fold`
- `distinct`, `merge`, `combine`, `sample`
- `changes`, `start_with`, `flat_map_switch`

### Required behavioral specification and tests

- Diamond-shaped graphs never produce inconsistent intermediate state.
- Multiple writes in one transaction have documented, deterministic behavior.
- Re-entrant writes are queued and never corrupt propagation.
- Observables start when their first observer appears and stop after the last
  observer is removed.
- Killing an `Owner` removes every subscription it owns.
- Restarted signals and streams have explicitly tested semantics.
- Observer panics do not leave the scheduler in a corrupted transaction state.
- Cycles are rejected or handled by an explicitly documented scheduling rule.

### Exit criteria

- Pure-Rust unit tests cover each semantic guarantee.
- No unsafe code is needed in `yse-model`.
- The API can express a counter, derived state, validation, and event sampling
  without interior-mutability boilerplate at call sites.

## Phase 2: `yse-ui` minimum viable UI layer

### Objective

Bind `yse-model` to a retained Qt Widgets tree with automatic lifecycle
ownership.

### Initial widgets and layouts

- Application and window
- Row, column, grid, and spacer
- Label and button
- Line edit and checkbox
- Signal-to-property bindings
- Widget signal-to-`EventStream` bindings
- Controlled and uncontrolled input patterns

### Required UX and safety work

- Every component creates an `Owner` tied to its Qt object lifetime.
- Updating a bound property occurs only on the GUI thread.
- Two-way bindings avoid feedback loops and preserve cursor-friendly text input.
- The public API exposes a narrow escape hatch to the underlying Qt object.

### Demonstration application

Build a validated settings form with derived state, validation messages,
enabled/disabled submit logic, and automatic cleanup when the window closes.

### Exit criteria

- No manual connection cleanup is needed in normal application code.
- Closing the window releases all of its subscriptions in a test.
- The form is concise enough to demonstrate a material improvement over raw
  CXX-Qt connections.

## Phase 3: Desktop application primitives

### Objective

Cover the primitives required by data-rich, traditional desktop software.

### Scope

- Actions, menus, toolbars, shortcuts, and standard dialogs
- Settings persistence and application commands
- Async task API with cancellation and UI-thread delivery
- Incremental list, selection, and table models
- Filtering, sorting, and large-table update benchmarks
- Undo/redo integration
- Structured logging and reactive graph diagnostics

### Demonstration application

Build a small data browser: asynchronously load records, filter and sort a
table, edit a form, show progress/error states, persist settings, and expose
menu shortcuts.

### Exit criteria

- Table updates are incremental; the widget tree is not rebuilt for every
  signal update.
- Async cancellation and error presentation are predictable and tested.
- The example is usable on Linux, Windows, and macOS.
- The project can explain why a widget changed through useful diagnostics.

## Phase 4: developer toolchain and distribution

### Objective

Make the supported path easy to create, test, package, and release.

### `yse-tool` commands

```text
cargo yse new <name>
cargo yse dev
cargo yse test
cargo yse bundle
```

### Responsibilities

- Generate a Rust workspace, metadata, icon placeholders, and CI workflow.
- Find or install a compatible Qt SDK through a documented, license-aware flow.
- Pin the Qt version, compiler family, and target architecture in project
  configuration.
- Run Cargo, CXX-Qt generation, and any necessary native build steps behind one
  predictable command.
- Produce native builds on Linux, Windows, and macOS CI runners.
- Invoke Qt deployment tooling: `windeployqt`, `macdeployqt`, and Qt's CMake
  deployment APIs on Linux.
- Generate artifacts appropriate to the platform: portable Linux bundle or
  package, Windows installer/archive, and signed/notarization-ready macOS app
  bundle.

### Exit criteria

- A new project builds from a clean supported machine using documented steps.
- CI produces working artifacts for all three desktop platforms.
- A release guide clearly identifies what remains application-specific: code
  signing credentials, notarization, store submissions, and third-party native
  dependencies.

## Deferred work

The following are intentionally not part of Phases 0-4:

- QML and Kirigami support
- Full KDE Framework bindings
- Mobile and web targets
- Complete coverage of the Qt Widgets API
- Visual UI designer integration
- A concurrent / cross-thread shared signal graph
- A stable 1.0 API guarantee

## Ongoing validation

Every phase should include feedback from Rust desktop developers, Qt teams, and
KDE contributors. Track completed external examples, successful clean-machine
builds, support requests, and production pilots rather than stars alone.
