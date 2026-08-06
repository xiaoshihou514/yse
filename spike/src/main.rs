mod bridge;

// The spike's Rust code does not use cxx-qt-lib types directly, but CXX-Qt's
// crate-initialization chain requires its native library to be linked.
// See https://kdab.github.io/cxx-qt/book/common-issues.html
#[allow(unused_extern_crates)]
extern crate cxx_qt_lib;

fn main() {
    // Entry point into the C++ shim; blocks in the Qt event loop.
    let exit_code = bridge::qobject::run_app();
    std::process::exit(exit_code);
}
