[windows]
set shell := ["powershell.exe", "-NoLogo", "-Command"]
[windows]
export CARGO_TARGET_DIR := `"$env:USERPROFILE/.cargo/target/yse"`

default:
    @just --list

# Format all Rust sources.
fmt:
    cargo fmt

# Check formatting without changing files.
fmt-check:
    cargo fmt --check

# Run the workspace lint gate.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run all unit, integration, and documentation tests.
test:
    cargo test --workspace

# Run the complete local validation gate.
check:
    bash scripts/architecture-check.sh
    bash scripts/qt-test-thread-check.sh
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Verify the declared minimum supported Rust version without network access.
msrv:
    @toolchain=1.89.0; \
      if ! rustup toolchain list | grep -q '^1\.89\.0'; then toolchain=stable; fi; \
      version="$(rustup run "$toolchain" rustc --version)"; \
      case "$version" in 'rustc 1.89.'*) ;; *) echo "Rust 1.89 toolchain is required for the MSRV gate (found $version)" >&2; exit 1;; esac; \
      CCACHE_DISABLE=1 rustup run "$toolchain" cargo check --workspace --all-targets --offline

# Build every workspace crate.
build:
    cargo build --workspace

# Exercise create -> test -> run -> bundle without sudo or a display server.
linux-smoke:
    bash scripts/linux-smoke.sh

# Run every supported Linux demo through its headless interaction path.
examples-smoke:
    bash scripts/examples-smoke.sh

# Run every supported Linux demo through a live Wayland session.
examples-wayland-smoke:
    QT_QPA_PLATFORM=wayland bash scripts/examples-smoke.sh

# Check repeated Qt tree teardown under Valgrind.
memory-smoke:
    bash scripts/memory-smoke.sh

# Verify locked dependency and CXX-Qt license declarations.
license-smoke:
    bash scripts/license-smoke.sh

# Prevent parallel QApplication cases inside one Rust test binary.
qt-test-thread-check:
    bash scripts/qt-test-thread-check.sh

# Enforce the Widgets-only API and workspace dependency direction.
architecture-check:
    bash scripts/architecture-check.sh

# Assemble and compile every publishable crate without publishing it.
package-smoke:
    bash scripts/package-smoke.sh

# Run every Fedora-local pre-release gate without publishing artifacts.
release-check: check msrv license-smoke package-smoke linux-smoke examples-smoke memory-smoke
    @echo "Fedora-local release checks passed; other platforms and clean hosts remain separate validation requirements."

example name:
    @if [ -f crates/yse-ui/examples/{{name}}.rs ]; then \
        cargo run -p yse-ui --example {{name}}; \
    else \
        cargo run -p yse-{{name}}; \
    fi

# --- Windows (host) recipes, PowerShell --------------------------------------

# setup
# Set up host prerequisites; run Rust toolchain bootstrap with `gansi setup`.
# diag
# Print the Windows environment and repo reachability.
# build
# Build the workspace on Windows (MSVC + Qt).
# Run a Windows helper action: setup | diag | build | check | smoke | bundle-smoke.
windows action:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 {{action}}

# Run a yse-ui example or workspace binary on Windows (e.g. `just windows-example settings`
# or `just windows-example taskmgr`).
windows-example name:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 example {{name}}

# Run any workspace binary on Windows (e.g. `just windows-run yse-taskmgr`).
windows-run name:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 run {{name}}
