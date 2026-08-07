//! Date/time edit wrappers with reactive value bindings.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, date_value_bindings, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink, Var};

pub struct DateTimeEdit {
    pub(crate) inner: Rc<Component>,
}

impl DateTimeEdit {
    /// Return the selected local date and time in ISO 8601 form.
    pub fn value(&self) -> String {
        unsafe { ffi::datetime_edit_value(self.inner.raw()) }
    }

    /// Set an ISO 8601 date/time value.
    pub fn set_value(&self, value: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::datetime_edit_set_value(self.inner.raw(), &value.into()) };
        }
    }
}

widget_wrapper!(DateTimeEdit);

/// A Qt calendar date editor with a popup calendar.
pub struct DateEdit {
    pub(crate) inner: Rc<Component>,
}

impl DateEdit {
    /// Return the selected date in ISO 8601 form.
    pub fn value(&self) -> String {
        unsafe { ffi::date_edit_value(self.inner.raw()) }
    }

    /// Set an ISO 8601 date value.
    pub fn set_value(&self, value: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::date_edit_set_value(self.inner.raw(), &value.into()) };
        }
    }
}

date_value_bindings!(DateEdit);
widget_wrapper!(DateEdit);

/// A Qt time editor with dedicated hour and minute spin controls.
pub struct TimeEdit {
    pub(crate) inner: Rc<Component>,
}

impl TimeEdit {
    /// Return the selected local time as `HH:mm`.
    pub fn value(&self) -> String {
        unsafe { ffi::time_edit_value(self.inner.raw()) }
    }

    /// Set a `HH:mm` time value.
    pub fn set_value(&self, value: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::time_edit_set_value(self.inner.raw(), &value.into()) };
        }
    }
}

date_value_bindings!(TimeEdit);
widget_wrapper!(TimeEdit);
