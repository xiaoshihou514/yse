# Roadmap implementation status

This document maps the deliverables in [`agent_docs/PLAN.md`](../agent_docs/PLAN.md)
to current evidence. “Implemented” means the source and focused tests exist;
“verified” names the environment on which that behavior has actually run.
Platform support is not inferred from conditional code or a successful build on
another operating system.

## Phase 0: CXX-Qt Widgets feasibility

| Requirement | Current evidence | Status |
|---|---|---|
| Start `QApplication` and open a Widgets window | All eight application smoke flows construct and run real Qt widget trees | Implemented; Fedora offscreen and Wayland verified |
| Label, button, layout, and Rust callback/update | Generated-project smoke clicks a Qt button and requires `count=1` | Implemented; Fedora verified |
| Reliable `QObject::destroyed` lifecycle hook | Lifecycle, soak, and Valgrind teardown gates exercise automatic owner cleanup | Implemented; Fedora verified |
| Background delivery on the GUI thread | Settings and data-browser smoke flows deliver worker results through `QtGuiScheduler` | Implemented; Fedora verified |
| Linux, Windows, and macOS smoke | Linux gates pass; Windows helpers exist; macOS remains experimental | Linux verified; Windows and macOS unverified |

The feasibility decision is positive on Linux: ordinary widgets follow a
repeatable thin-shim pattern and application projects do not generate their own
C++ or QML. The workspace gate rejects QML/Qt Quick types in the bridge and
generated application surface, and enforces the intended facade dependency
direction.

## Phase 1: reactive runtime

The complete initial operator surface and every named semantic guarantee have
focused pure-Rust tests: transactional diamond propagation, deterministic
batched writes, queued re-entrant writes, start/stop behavior, owner teardown,
restart semantics, panic recovery, and cycle rejection. `yse-model` forbids
unsafe code and is checked at Rust 1.89.

**Status:** implemented and Fedora-verified. This phase has no Qt or desktop
platform dependency.

## Phase 2: minimum viable UI layer

The initial widgets, layouts, one-way and two-way bindings, Qt-object escape
hatches, feedback-loop guards, lifecycle ownership, and settings-form
demonstration are implemented. Focused DSL cases cover both controlled inputs
backed by a `Var` and uncontrolled inputs whose state remains in the Qt widget.
Compile-time assertions prevent reactive owners, variables, windows, and input
handles from implementing `Send` or `Sync`. Qt integration binaries
structurally expose one Rust harness test apiece so `QApplication` remains on
one test thread.

**Status:** implemented; Fedora offscreen and Wayland verified. Other operating
systems remain unverified.

## Phase 3: desktop primitives

Actions, menus, toolbars, shortcuts, dialogs, settings, cancellable background
tasks, incremental list/table models, sorting/filtering benchmarks, undo/redo,
logging, and graph diagnostics are present. The data-browser smoke drives load,
cancel, failure, filter, sort, edit, persistence, and shortcut paths without
rebuilding the table widget tree. It asserts the cancellation state before
starting the failure case, then requires the final user-visible error and table
contents.

**Status:** implemented; Fedora offscreen and Wayland verified. The roadmap's
three-platform example exit criterion is not yet met.

## Phase 4: toolchain and distribution

`gansi` creates, initializes, diagnoses, runs, tests, analyzes, formats, updates,
cleans, and bundles applications. The Linux gate verifies toolchain pins,
offline local scaffolding, deterministic archives, relocation, bundled Qt
loading, desktop/AppStream metadata, dependency inventories, Qt/CXX-Qt license
materials, and GLIBC requirements. All publishable crates are assembled and
compiled from standalone package archives without uploading them.

Generated applications include a Linux GitHub Actions workflow that checks out
Yse directly and replaces the scaffold's machine-local dependency path, so it
does not require published crates. The workflow itself remains unverified until
it runs on a hosted runner.

**Status:** Linux implementation is Fedora-verified. The following original
exit criteria remain open:

- clean Ubuntu or oldest-supported-distribution installation;
- hosted CI artifacts;
- Windows execution and deployment verification;
- macOS execution, deployment, signing, and notarization readiness.

macOS is deliberately deferred until a test host is available. The repository
must not turn conditional macOS code into a support claim in the meantime.

## Reproducible local evidence

Run the complete non-publishing, display-free Fedora gate with:

```sh
CCACHE_DISABLE=1 just release-check
```

When a live Wayland compositor is available, additionally run:

```sh
just examples-wayland-smoke
```

See [`validation.md`](validation.md) for exact host versions, test coverage,
known compatibility limits, and license-gate scope.
