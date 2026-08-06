//! Internal component state shared by every widget wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use yse_model::{Owner, Sink};

/// Internal component state shared by every widget wrapper.
#[doc(hidden)]
pub struct Component {
    pub ptr: *mut ffi::Widget,
    pub owner: RefCell<Owner>,
    pub destroyed: Cell<bool>,
    pub updating: Cell<bool>,
    pub clicked_sink: RefCell<Option<Rc<Sink<()>>>>,
    pub text_sink: RefCell<Option<Rc<Sink<String>>>>,
    pub toggled_sink: RefCell<Option<Rc<Sink<bool>>>>,
}

impl Component {
    /// # Safety
    ///
    /// `ptr` must be a widget pointer created by the shim and must not be
    /// shared with any other `Component`.
    pub unsafe fn from_raw(ptr: *mut ffi::Widget) -> Rc<Self> {
        let this = Rc::new(Self {
            ptr,
            owner: RefCell::new(Owner::new()),
            destroyed: Cell::new(false),
            updating: Cell::new(false),
            clicked_sink: RefCell::new(None),
            text_sink: RefCell::new(None),
            toggled_sink: RefCell::new(None),
        });
        unsafe { ffi::widget_set_destroyed_cb(ptr, &*this as *const Self as *mut Void) };
        this
    }

    pub fn raw(&self) -> *mut ffi::Widget {
        self.ptr
    }

    pub fn is_alive(&self) -> bool {
        !self.destroyed.get()
    }

    pub fn clear_owner(&self) {
        self.owner.borrow_mut().clear();
    }

    pub fn on_clicked(&self) {
        if !self.is_alive() {
            return;
        }
        if let Some(sink) = self.clicked_sink.borrow().as_ref() {
            sink.send(());
        }
    }

    pub fn on_text_changed(&self) {
        if !self.is_alive() || self.updating.get() {
            return;
        }
        let text = unsafe { ffi::line_edit_text(self.ptr) };
        if let Some(sink) = self.text_sink.borrow().as_ref() {
            sink.send(text);
        }
    }

    pub fn on_toggled(&self) {
        if !self.is_alive() || self.updating.get() {
            return;
        }
        let checked = unsafe { ffi::checkbox_checked(self.ptr) };
        if let Some(sink) = self.toggled_sink.borrow().as_ref() {
            sink.send(checked);
        }
    }

    pub fn on_destroyed(&self) {
        self.destroyed.set(true);
        self.clear_owner();
    }

    /// Programmatic text write for controlled bindings: suppresses the echo
    /// through `text_changed` so two-way bindings cannot loop.
    pub fn set_text_guarded(&self, text: String) {
        if !self.is_alive() {
            return;
        }
        self.updating.set(true);
        unsafe { ffi::line_edit_set_text(self.ptr, &text) };
        self.updating.set(false);
    }

    pub fn set_label_text(&self, text: String) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::label_set_text(self.ptr, &text) };
    }

    pub fn set_button_enabled(&self, enabled: bool) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::widget_set_enabled(self.ptr, enabled) };
    }

    pub fn set_checkbox_checked(&self, checked: bool) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::checkbox_set_checked(self.ptr, checked) };
    }
}

impl Drop for Component {
    fn drop(&mut self) {
        unsafe { ffi::widget_drop(self.ptr) };
    }
}
