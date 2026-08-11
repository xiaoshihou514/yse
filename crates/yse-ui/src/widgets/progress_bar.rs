//! Progress bar wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

/// A retained progress indicator widget.
pub struct ProgressBar {
    pub(crate) inner: Rc<Component>,
}

impl ProgressBar {
    /// The current value.
    pub fn value(&self) -> i32 {
        unsafe { ffi::widget_value(self.inner.raw()) }
    }

    /// Set the value (clamped to the current range).
    pub fn set_value(&self, value: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_value(self.inner.raw(), value) };
        }
    }

    /// Set the accepted value range.
    pub fn set_range(&self, min: i32, max: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::progress_set_range(self.inner.raw(), min, max) };
        }
    }

    /// Bind the value to a signal.
    pub fn bind_value(&self, signal: &Signal<i32>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |value| {
            if let Some(this) = weak.upgrade() {
                this.set_widget_value(*value);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(ProgressBar);
