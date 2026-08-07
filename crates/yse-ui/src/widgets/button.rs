//! button wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink};

pub struct Button {
    pub(crate) inner: Rc<Component>,
}

impl Button {
    /// Use an icon from the active platform icon theme.
    pub fn set_icon(&self, theme_name: impl AsRef<str>) {
        if self.inner.is_alive() {
            unsafe { ffi::button_set_icon(self.inner.raw(), theme_name.as_ref()) };
        }
    }

    /// Set an icon-theme name and return the button for chaining.
    pub fn icon(self, theme_name: impl AsRef<str>) -> Self {
        self.set_icon(theme_name);
        self
    }

    /// A stream of click events.
    pub fn clicked(&self) -> EventStream<()> {
        if self.inner.clicked_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_clicked_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.clicked_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.clicked_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a click handler. The subscription lives in the button's
    /// `Owner` and is released with the widget tree.
    pub fn on_click<F>(&self, f: F) -> &Self
    where
        F: FnMut(&()) + 'static,
    {
        self.inner.owner.borrow_mut().add(self.clicked().observe(f));
        self
    }

    /// Enable or disable the button.
    pub fn set_enabled(&self, enabled: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_enabled(self.inner.raw(), enabled) };
        }
    }

    /// Bind the enabled state to a signal.
    pub fn bind_enabled(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |enabled| {
            if let Some(this) = weak.upgrade() {
                this.set_button_enabled(*enabled);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Simulate a click (used by tests and headless smoke runs).
    pub fn click(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::button_click(self.inner.raw()) };
        }
    }

    /// Change the button's label.
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::button_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// Bind the button's label to a signal. The binding lives in the button's
    /// owner and is released with the widget tree.
    pub fn bind_text(&self, signal: &Signal<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_button_text((*text).clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// The button's current label.
    pub fn text(&self) -> String {
        unsafe { ffi::button_text(self.inner.raw()) }
    }
}

widget_wrapper!(Button);
