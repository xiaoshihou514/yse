//! label wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

/// A retained text label widget.
pub struct Label {
    pub(crate) inner: Rc<Component>,
}

impl Label {
    /// Set the label text.
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::label_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// Read the current label text.
    pub fn text(&self) -> String {
        unsafe { ffi::label_text(self.inner.raw()) }
    }

    /// Bind the label text to a signal. The binding lives in the component's
    /// owner and is released when the widget is destroyed.
    pub fn bind_text(&self, signal: &Signal<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_label_text(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(Label);
