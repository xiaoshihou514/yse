//! spin_box wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, value_two_way, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink, Var};

pub struct SpinBox {
    pub(crate) inner: Rc<Component>,
}

impl SpinBox {
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
            unsafe { ffi::spin_box_set_range(self.inner.raw(), min, max) };
        }
    }

    /// A stream of value changes carrying the new value.
    pub fn value_changed(&self) -> EventStream<i32> {
        if self.inner.value_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_value_changed_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.value_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.value_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a value-change handler carrying the new value.
    pub fn on_value_change<F>(&self, f: F) -> &Self
    where
        F: FnMut(&i32) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.value_changed().observe(f));
        self
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

value_two_way!(SpinBox);
widget_wrapper!(SpinBox);
