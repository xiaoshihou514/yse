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
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Build every workspace crate.
build:
    cargo build --workspace

# --- Windows (host) recipes, PowerShell --------------------------------------

# Set up Qt 6.8.3 on Windows via aqt (install through `uv tool install aqtinstall`).
windows-setup:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 setup

# Print the Windows environment and repo reachability.
windows-diag:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 diag

# Build the workspace on Windows (MSVC + Qt).
windows-build:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 build

# Run a yse-ui example as a real window on Windows (e.g. `just windows-example settings`).
windows-example name:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 example {{name}}

# Run the full local gate on Windows.
windows-check:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 check
