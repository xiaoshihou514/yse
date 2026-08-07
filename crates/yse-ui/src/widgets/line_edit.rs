//! line_edit wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink, Var};

pub struct LineEdit {
    pub(crate) inner: Rc<Component>,
}

impl LineEdit {
    /// The current text.
    pub fn text(&self) -> String {
        unsafe { ffi::line_edit_text(self.inner.raw()) }
    }

    /// Set the text programmatically (user edits are observable via
    /// [`LineEdit::text_changed`]).
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::line_edit_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// A stream of user text changes.
    pub fn text_changed(&self) -> EventStream<String> {
        if self.inner.text_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_text_changed_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.text_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.text_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a text-change handler (user edits; programmatic writes made
    /// through controlled bindings are suppressed).
    pub fn on_text_change<F>(&self, f: F) -> &Self
    where
        F: FnMut(&String) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.text_changed().observe(f));
        self
    }

    /// Controlled binding: the widget text follows the signal.
    pub fn bind_text(&self, signal: &Signal<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_text_guarded(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Two-way binding between the widget text and a `Var`, with a guard that
    /// prevents feedback loops.
    pub fn bind_text_two_way(&self, var: &Var<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = var.signal().observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_text_guarded(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);

        let var_rc = var.clone();
        let subscription = self.text_changed().observe(move |text| {
            if var_rc.value().as_ref() != text {
                var_rc.set(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(LineEdit);
