# Safety and FFI invariants

Yse crosses the Rust/C++ boundary on every widget interaction. This document
records the invariants that keep that crossing safe, and the rules for
extending it.

## Threading model

- The reactive graph and every widget live on the Qt GUI thread. Rust objects
  behind it are `Rc`/`RefCell`/raw-pointer based and are **not** `Send` or
  `Sync`.
- Background work must return to the GUI thread through a `Scheduler`:
  `yse_model::QueueScheduler` in tests, `yse_ui::QtGuiScheduler` in the app.
  Results are delivered inside a transaction on the GUI thread.
- Delayed work must go through a `Timer` (`yse_ui::QtTimer` in the app,
  `yse_model::ManualTimer` in tests), never through a thread that touches
  widgets directly.

## Ownership and pointer lifetime

- The C++ side allocates a `Widget` struct per Qt widget; Rust wraps it in an
  `Rc<Component>`. `Component::Drop` calls `widget_drop`, which disconnects
  only the connections Yse installed (never a wildcard disconnect), nulls its
  callback data, and deletes the `QWidget` if Yse owns it and it is still
  alive.
- The Rust component tree mirrors Qt parentage (strong parent → child, weak
  child → parent). `adopt_child` must accompany every public `add` API so Qt
  reparenting through layouts does not strand Rust state.
- The `QObject::destroyed` hook fires during native teardown, *before* the
  native memory is freed. It marks the component dead (`is_alive() == false`),
  clears its `Owner` (releasing every subscription), clears retained state and
  children, and detaches it from its parent. All `on_*` component methods
  therefore start with an `is_alive()` check, and every C++ shim function
  guards on `w->alive` and `w->q != nullptr`.
- Raw pointers handed to C++ (callback data, selection bridges, model
  pointers) are only valid while the wrapping `Rc` is retained — by the
  component tree or an explicit `retain` — and the native widget has not been
  destroyed. The destroyed hook dissociates Rust state before Qt frees the
  object, so a queued callback that fires late is a safe no-op.

## Panic containment at the FFI boundary

- CXX converts a Rust panic that unwinds into C++ into a `rust::Error`
  exception; uncaught in the Qt event loop that terminates the application.
  Every `extern "Rust"` entry point in `crates/yse-ui/src/lib.rs` is therefore
  wrapped in `catch_callback`, which logs the panic (structured logger plus
  stderr) and swallows it.
- `yse_model::transaction` also catches panics internally: it settles pending
  writes, clears transaction bookkeeping, and resumes the unwind, so a panic
  in an observer never leaves the graph mid-transaction.
- **Rule:** any new `extern "Rust"` callback (widget signal, dialog result,
  timer, scheduled task) must be wrapped in `catch_callback` in `lib.rs`.
  Panics inside user handlers are expected and must never escape.

## Callback echo and feedback loops

- Two-way line-edit bindings set an `updating` guard around programmatic
  writes so the Qt `textChanged` echo does not re-enter the binding. New
  two-way widget bindings need the same guard.
- `line_edit_set_text` flows through the `TextState` QObject; programmatic
  writes do fire `textChanged` (suppressed only inside controlled bindings),
  so a `text_changed()` observer sees them.

## Extending the widget set

Every new widget follows the same five-part pattern:

1. shim functions in `crates/yse-ui/src/widgets.h` + `widgets.cpp`
2. bridge declarations in the `#[cxx::bridge]` module in `lib.rs`
3. a Rust wrapper (with bindings and handlers) in `lib.rs`
4. a DSL factory in `dsl.rs` if it fits the `View` tree
5. headless tests in `crates/yse-ui/tests/` and a smoke run

The shim owns construction and Qt-native state; Rust owns reactive state and
lifecycle. Never add unsafe code to `yse-model` (it is
`#![forbid(unsafe_code)]`); unsafe lives only in `yse-ui`.

## Testing

- All UI tests run with `QT_QPA_PLATFORM=offscreen`; they share the one
  `QApplication` created by the C++ shim, so keep cross-thread widget access
  out of test code.
- `crates/yse-ui/tests/panic_safety.rs` asserts that panicking click, text,
  and selection handlers are contained and that healthy handlers still run
  afterwards.
- The local gate is `cargo test --workspace`, clippy with `-D warnings`,
  `cargo fmt --check`, and `cargo doc --workspace --no-deps`; run it before
  every commit.
