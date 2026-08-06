// Phase 0 C++ shim for Yse.
//
// Rust owns the reactive state; this header is the only hand-written C++
// surface the Rust side sees.

#pragma once

// Entry point called from Rust. Blocks in the Qt event loop and returns the
// application exit code.
int run_app() noexcept;
