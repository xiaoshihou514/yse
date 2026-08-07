//! Checkbox wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink};

pub struct CheckBox {
    pub(crate) inner: Rc<Component>,
}

impl CheckBox {
    /// Whether the box is checked.
    pub fn checked(&self) -> bool {
        unsafe { ffi::checkbox_checked(self.inner.raw()) }
    }

    /// Set the checked state programmatically.
    pub fn set_checked(&self, checked: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::checkbox_set_checked(self.inner.raw(), checked) };
        }
    }

    /// A stream of toggle events carrying the new checked state.
    pub fn toggled(&self) -> EventStream<bool> {
        if self.inner.toggled_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_toggled_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.toggled_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.toggled_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a toggle handler carrying the new checked state.
    pub fn on_toggle<F>(&self, f: F) -> &Self
    where
        F: FnMut(&bool) + 'static,
    {
        self.inner.owner.borrow_mut().add(self.toggled().observe(f));
        self
    }

    /// Bind the checked state to a signal.
    pub fn bind_checked(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |checked| {
            if let Some(this) = weak.upgrade() {
                this.set_checkbox_checked(*checked);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(CheckBox);
