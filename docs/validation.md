# Platform validation

This file records reproducible evidence rather than intended support.

## Verified locally

Environment:

- Fedora Linux 44 Workstation, `x86_64`
- Rust 1.89.0
- GCC 16.1.1 and GNU ld 2.46
- CMake 4.3.0, Ninja 1.13.2, pkg-config 2.5.1
- System Qt 6 with platform plugins under `/usr/lib64/qt6/plugins`
- Wayland login session; Qt smoke applications use the offscreen platform
- No sudo and no network access during validation

Validated commands:

```sh
CCACHE_DISABLE=1 just check
just msrv
just linux-smoke
just examples-smoke
just examples-wayland-smoke
just memory-smoke
just license-smoke
just package-smoke
CCACHE_DISABLE=1 just release-check
```

`just release-check` is the non-publishing aggregate of all display-free
Fedora-local commands above. The live Wayland gate remains separate because it
requires an active compositor session. Passing the aggregate does not broaden
the platform claims in this document.

`just linux-smoke` exercises the default release bundle. Set
`YSE_SMOKE_BUNDLE_PROFILE=debug` only for a faster local packaging iteration.
The gate rebuilds the bundle and requires identical archive checksums.

The current Fedora 44 release-smoke artifact reports a bundle-wide requirement
of GLIBC 2.43. This proves the archive is relocatable on the build host, not
compatible with older Linux distributions. Produce release artifacts on the
oldest supported build environment and confirm the generated
`GLIBC_REQUIREMENTS.tsv` before making a compatibility claim.

The workspace gate covers formatting, Clippy with warnings denied, workspace
tests, doctests, rustdoc with warnings denied, an architecture check that keeps
QML/Qt Quick out of the implementation and generated application API, and a
structural check that each Qt integration binary exposes only one Rust harness
test so `QApplication` never moves between parallel test workers. The Linux smoke test uses a
temporary generated project, the workspace Cargo target cache, disabled
incremental compilation, and offline Cargo mode to stay within a no-sudo user
quota while exercising:

Both public implementation crates enable Rust's `missing_docs` lint, so newly
added public model or widget APIs cannot silently bypass API documentation.

1. Isolated global configuration plus offline registration of the system Qt SDK
   through `gansi config` and `gansi setup`
2. `gansi doctor --verbose`, resolving Qt from the registered SDK root
3. Idempotent `gansi init` in an existing source directory, preserving a
   pre-existing `.gitignore` byte-for-byte
4. Safe rejection of unpublished registry scaffolding followed by
   `gansi create --local <checkout>`
5. Detection and enforcement of the generated Qt 6.11.1, GCC, and `x86_64`
   toolchain pins
6. Generated-project model tests
7. Generated-project formatting, Clippy analysis with warnings denied, an
   offline optional dependency addition, and a Cargo.lock upgrade through
   `gansi`
8. Offscreen execution with a real Qt button click and `count=1` assertion
9. Release bundle creation with third-party notices, Qt license texts, locked
   Cargo license inventory including CXX-Qt, plus Qt and system runtime
   inventories
10. Creation and SHA-256 verification of the architecture-labelled Linux
   `.tar.gz`
11. Extraction and execution through the relocated bundle launcher, with
   dynamic-loader output
   proving `libQt6Core.so.6` is loaded from the extracted payload
12. Validation of generated and bundled freedesktop desktop entries, plus the
    scalable icon installation layout; the smoke changes `bundle_name` and
    `app_id` after generation and verifies both values in the bundle
13. `gansi clean` removal of `dist/` and an isolated absolute
    `CARGO_TARGET_DIR`

The examples gate runs `settings`, `controls`, `data_browser`, `kcalc`,
`kalarm`, `filelight`, `media-converter`, and `taskmgr`. The media-converter
smoke additionally creates a WAV input and verifies a non-empty FLAC output
produced through the FFmpeg library API.

All eight examples also pass their deterministic interaction flows through the
live Fedora Wayland session (not the offscreen platform). This is native-display
runtime evidence, not a complete desktop-integration matrix.

The `yse-ui` repeated window teardown test also passes Valgrind Memcheck with
memory errors and definite leaks treated as failures. Two narrow suppressions
cover Fedora Qt 6.11's `QObject::connect` SIMD overread report during
`QApplication` initialization and its FreeType thread-cleanup allocation;
unsuppressed Yse and CXX-Qt findings still fail the gate.

The license smoke checks the complete locked Cargo graph for missing license
metadata or license files. It additionally requires every workspace package
and every locked CXX/CXX-Qt package to declare `MIT OR Apache-2.0`.

All four publishable crates also pass standalone `cargo package --allow-dirty
--offline` verification. `yse-ui` and `yse` are verified in publication order
against the locally packaged `yse-model` and `yse-ui` sources through temporary
Cargo patches. The gate also inspects each `.crate` archive for required Rust,
C++, build-script, and license files. Nothing is uploaded.

## Not yet verified

- A clean Ubuntu installation
- X11 sessions
- KDE Plasma and GNOME integration details such as native dialogs, themes,
  clipboard, desktop files, and fractional scaling
- Linux bundle compatibility on a distribution older than its Fedora build host;
  the bundle carries Qt but intentionally relies on target glibc, graphics
  drivers, and other non-Qt system libraries
- Static Qt builds, which are not Yse's default open-source licensing path
- Commercial Qt builds or legal compliance of any specific application

These items require another host, session, Qt license context, or distribution
artifact and must not be inferred from the Fedora offscreen result.
