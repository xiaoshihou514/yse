//! Phase 0 bridge: a Rust-owned QObject that the C++ shim drives.

use cxx_qt::{CxxQtType, Threading};
use std::pin::Pin;

#[cxx_qt::bridge(namespace = "yse::spike")]
pub mod qobject {
    #[namespace = ""]
    unsafe extern "C++" {
        include!("yse-spike/src/spike.h");

        /// Run the Phase 0 spike application (blocks until the event loop exits).
        fn run_app() -> i32;
    }

    impl cxx_qt::Threading for Counter {}

    extern "RustQt" {
        #[qobject]
        #[qproperty(u32, count)]
        #[qproperty(u32, result)]
        type Counter = super::CounterRust;

        #[qinvokable]
        #[cxx_name = "incrementCount"]
        fn increment_count(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "startBackgroundWork"]
        fn start_background_work(self: Pin<&mut Self>);
    }
}

/// Rust state behind the `Counter` QObject.
///
/// `_lifecycle` prints when the owning QObject is destroyed, proving that Rust
/// state is released together with the widget tree.
pub struct CounterRust {
    pub count: u32,
    pub result: u32,
    _lifecycle: LifecycleMarker,
}

struct LifecycleMarker;

impl Drop for LifecycleMarker {
    fn drop(&mut self) {
        println!("[spike] CounterRust dropped: Rust state released with its QObject");
    }
}

impl Default for CounterRust {
    fn default() -> Self {
        Self {
            count: 0,
            result: 0,
            _lifecycle: LifecycleMarker,
        }
    }
}

impl qobject::Counter {
    /// Invokable: a button click reached Rust and bumps the property; C++
    /// observes the change through the generated `countChanged` signal.
    pub fn increment_count(mut self: Pin<&mut Self>) {
        let next = self.as_mut().rust_mut().count + 1;
        self.as_mut().set_count(next);
        println!("[spike] increment_count: {next}");
    }

    /// Invokable: starts real background work and dispatches the result back
    /// to the GUI thread through the object's `CxxQtThread`.
    pub fn start_background_work(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            // Simulated off-GUI-thread work.
            std::thread::sleep(std::time::Duration::from_millis(300));
            let answer: u32 = 6 * 7;
            let delivered = qt_thread
                .queue(move |mut counter| {
                    counter.as_mut().set_result(answer);
                })
                .is_ok();
            println!("[spike] background result {answer} delivered on GUI thread: {delivered}");
        });
    }
}
