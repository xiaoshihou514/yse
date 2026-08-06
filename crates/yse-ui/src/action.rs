//! Internal state for QAction wrappers.

use crate::bridge as ffi;
use crate::bridge::Void;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use yse_model::{Owner, Sink};

/// Internal state shared by every [`crate::Action`] wrapper.
#[doc(hidden)]
pub struct ActionState {
    ptr: *mut ffi::Action,
    pub owner: RefCell<Owner>,
    pub destroyed: Cell<bool>,
    pub triggered_sink: RefCell<Option<Rc<Sink<()>>>>,
}

impl ActionState {
    /// # Safety
    ///
    /// `ptr` must be an action pointer created by the shim and must not be
    /// shared with any other `ActionState`.
    pub unsafe fn from_raw(ptr: *mut ffi::Action) -> Rc<Self> {
        let this = Rc::new(Self {
            ptr,
            owner: RefCell::new(Owner::new()),
            destroyed: Cell::new(false),
            triggered_sink: RefCell::new(None),
        });
        unsafe { ffi::action_set_destroyed_cb(ptr, &*this as *const Self as *mut Void) };
        this
    }

    pub fn raw(&self) -> *mut ffi::Action {
        self.ptr
    }

    pub fn is_alive(&self) -> bool {
        !self.destroyed.get()
    }

    pub fn clear_owner(&self) {
        self.owner.borrow_mut().clear();
    }

    pub fn on_destroyed(&self) {
        self.destroyed.set(true);
        self.clear_owner();
    }

    pub fn on_triggered(&self) {
        if !self.is_alive() {
            return;
        }
        if let Some(sink) = self.triggered_sink.borrow().as_ref() {
            sink.send(());
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::action_set_enabled(self.ptr, enabled) };
    }
}

impl Drop for ActionState {
    fn drop(&mut self) {
        unsafe { ffi::action_drop(self.ptr) };
    }
}
