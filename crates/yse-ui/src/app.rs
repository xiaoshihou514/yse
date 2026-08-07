/// The Qt application object. Must be created before any window.
#[derive(Clone, Copy)]
pub struct Application;

// CXX-Qt emits generated C++ code as static archives. Build-script link flags
// are not retained when this crate is re-exported through `yse`, so retain the
// initializer entry points in the rlib metadata and call them from app setup.
// Each initializer is idempotent on the C++ side.
#[link(name = "yse-ui-cxxqt-generated", kind = "static")]
unsafe extern "C" {
    fn cxx_qt_init_crate_yse_ui() -> bool;
}
#[link(name = "cxx-qt-lib-cxxqt-generated", kind = "static")]
unsafe extern "C" {
    fn cxx_qt_init_crate_cxx_qt_lib() -> bool;
}
#[link(name = "cxx-qt-cxxqt-generated", kind = "static")]
unsafe extern "C" {
    fn cxx_qt_init_crate_cxx_qt() -> bool;
}

use crate::bridge as ffi;
use crate::bridge::Void;
use std::time::Duration;
use yse_model::Scheduler;

impl Application {
    /// Initialise the Qt application (idempotent).
    pub fn init() -> Self {
        // SAFETY: CXX-Qt generates these C ABI initialization functions for
        // exactly these linked static archives. They are internally guarded
        // with `std::once_flag`, so repeated application initialization is
        // safe and does not duplicate registration.
        unsafe {
            cxx_qt_init_crate_cxx_qt();
            cxx_qt_init_crate_cxx_qt_lib();
            cxx_qt_init_crate_yse_ui();
        }
        ffi::app_init();
        Self
    }

    /// Apply an application-wide Qt style sheet.
    ///
    /// Yse otherwise inherits the platform's palette, fonts, and widget
    /// style. Use an empty sheet (or [`Self::clear_style_sheet`]) to restore
    /// those defaults.
    pub fn set_style_sheet(&self, style_sheet: impl AsRef<str>) {
        ffi::app_set_style_sheet(style_sheet.as_ref());
    }

    /// Remove an application-wide style sheet and restore the platform style.
    pub fn clear_style_sheet(&self) {
        ffi::app_set_style_sheet("");
    }

    /// Whether the system currently requests a dark color scheme.
    pub fn system_dark(&self) -> bool {
        ffi::app_system_color_scheme_dark()
    }

    /// Apply a fixed light or dark color scheme (Fusion style + palette).
    pub fn apply_color_scheme(&self, dark: bool) {
        ffi::app_apply_color_scheme(dark);
    }

    /// Follow the system color scheme: applies the matching Fusion palette and
    /// returns whether the result is dark.
    pub fn follow_system_color_scheme(&self) -> bool {
        let dark = self.system_dark();
        self.apply_color_scheme(dark);
        dark
    }

    /// Run the Qt event loop; returns the application exit code.
    pub fn exec(&self) -> i32 {
        ffi::app_exec()
    }

    /// Schedule `QCoreApplication::quit` after `ms` milliseconds.
    pub fn quit_after(&self, ms: u64) {
        ffi::app_quit_after(ms as i32);
    }

    /// Run `f` on the GUI thread after `ms` milliseconds. If the app quits
    /// first, the closure is leaked (one-shot timer delivery).
    pub fn after<F>(&self, ms: u64, f: F)
    where
        F: FnOnce() + 'static,
    {
        let outer: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
        let ptr = Box::into_raw(outer) as *mut Void;
        unsafe { ffi::app_invoke_after(ms as i32, ptr) };
    }
}

/// A [`Scheduler`] that runs tasks on the Qt event loop, i.e. the GUI thread.
/// Use this as the delivery target for [`yse_model::spawn_task`].
pub struct QtGuiScheduler;

impl Scheduler for QtGuiScheduler {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
        // The closure is leaked to a thin pointer that Qt hands back on the
        // GUI thread, where on_gui_scheduled reconstructs and runs it. If the
        // app quits before the queued call runs, the closure leaks; this is
        // acceptable for one-shot deliveries.
        let outer: Box<Box<dyn FnOnce() + Send>> = Box::new(task);
        let ptr = Box::into_raw(outer) as *mut Void;
        unsafe { ffi::app_schedule_gui(ptr) };
    }
}

/// A [`yse_model::Timer`] backed by Qt's event loop, so delayed tasks run on
/// the GUI thread. Use it to drive `debounce`/`throttle`/`delay` operators in
/// a live application; [`yse_model::ManualTimer`] is the deterministic
/// equivalent for tests.
pub struct QtTimer;

impl yse_model::Timer for QtTimer {
    fn schedule_after(&self, delay: Duration, task: Box<dyn FnOnce()>) {
        // Same delivery path as `Application::after`: the closure is leaked to
        // a thin pointer that Qt hands back on the GUI thread, where
        // `on_app_timer` reconstructs and runs it. If the app quits first, the
        // closure leaks; acceptable for one-shot timer deliveries.
        let outer: Box<Box<dyn FnOnce()>> = Box::new(task);
        let ptr = Box::into_raw(outer) as *mut Void;
        unsafe { ffi::app_invoke_after(delay.as_millis() as i32, ptr) };
    }
}
