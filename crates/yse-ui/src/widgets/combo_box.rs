//! Combo box wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, value_two_way, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink, Var};

pub struct ComboBox {
    pub(crate) inner: Rc<Component>,
}

impl ComboBox {
    /// Replace the item list; the current index resets to the first item.
    pub fn set_items<I, S>(&self, items: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if self.inner.is_alive() {
            unsafe {
                ffi::combo_set_items(
                    self.inner.raw(),
                    items.into_iter().map(Into::into).collect(),
                )
            };
        }
    }

    /// The text of the currently selected item, or an empty string when the
    /// box has no selection.
    pub fn current_text(&self) -> String {
        unsafe { ffi::combo_current_text(self.inner.raw()) }
    }

    /// The index of the currently selected item, or `-1` when empty.
    pub fn current_index(&self) -> i32 {
        unsafe { ffi::widget_value(self.inner.raw()) }
    }

    /// Select the item at `index`. Out-of-range indices clear the selection.
    pub fn set_current_index(&self, index: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_value(self.inner.raw(), index) };
        }
    }

    /// Select the item whose text matches `text`; no-op when absent.
    pub fn set_current_text(&self, text: impl AsRef<str>) {
        if self.inner.is_alive() {
            unsafe { ffi::combo_set_current_text(self.inner.raw(), text.as_ref()) };
        }
    }

    /// A stream of selection changes carrying the new item index.
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

    /// Register a selection-change handler carrying the new item index.
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

    /// Bind the selected index to a signal.
    pub fn bind_value(&self, signal: &Signal<i32>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |index| {
            if let Some(this) = weak.upgrade() {
                this.set_widget_value(*index);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

value_two_way!(ComboBox);
widget_wrapper!(ComboBox);
