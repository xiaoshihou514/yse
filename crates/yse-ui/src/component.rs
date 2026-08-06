//! Internal component state shared by every widget wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use yse_model::{Owner, Sink};

pub(crate) type RetainedId = u64;

/// Internal component state shared by every widget wrapper.
#[doc(hidden)]
pub struct Component {
    pub ptr: *mut ffi::Widget,
    pub owner: RefCell<Owner>,
    pub destroyed: Cell<bool>,
    parent: RefCell<Option<Weak<Component>>>,
    self_weak: Weak<Component>,
    children: RefCell<Vec<Rc<Component>>>,
    retained: RefCell<HashMap<RetainedId, Rc<dyn Any>>>,
    next_retained_id: Cell<RetainedId>,
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
        unsafe { Self::from_raw_with_parent(ptr, None) }
    }

    /// # Safety
    ///
    /// `ptr` must be a newly-created child widget of `parent` and must not be
    /// shared with another `Component`.
    pub unsafe fn from_raw_child(ptr: *mut ffi::Widget, parent: &Rc<Self>) -> Rc<Self> {
        unsafe { Self::from_raw_with_parent(ptr, Some(parent)) }
    }

    unsafe fn from_raw_with_parent(ptr: *mut ffi::Widget, parent: Option<&Rc<Self>>) -> Rc<Self> {
        let this = Rc::new_cyclic(|self_weak| Self {
            ptr,
            owner: RefCell::new(Owner::new()),
            destroyed: Cell::new(false),
            parent: RefCell::new(parent.map(Rc::downgrade)),
            self_weak: self_weak.clone(),
            children: RefCell::new(Vec::new()),
            retained: RefCell::new(HashMap::new()),
            next_retained_id: Cell::new(0),
            updating: Cell::new(false),
            clicked_sink: RefCell::new(None),
            text_sink: RefCell::new(None),
            toggled_sink: RefCell::new(None),
        });
        unsafe { ffi::widget_set_destroyed_cb(ptr, &*this as *const Self as *mut Void) };
        if let Some(parent) = parent {
            parent.children.borrow_mut().push(this.clone());
        }
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

    /// Keep Rust state alive for as long as this native component can call it.
    /// The caller may release the value early once its native callback is done.
    pub(crate) fn retain<T: Any>(&self, value: Rc<T>) -> RetainedId {
        let id = self.next_retained_id.get();
        self.next_retained_id.set(id.wrapping_add(1));
        let value: Rc<dyn Any> = value;
        self.retained.borrow_mut().insert(id, value);
        id
    }

    pub(crate) fn release(&self, id: RetainedId) {
        self.retained.borrow_mut().remove(&id);
    }

    /// Move a component in the logical ownership tree after Qt reparents its
    /// widget through a layout. This must accompany every public `add` API:
    /// Qt parentage alone is not enough to keep Rust callback state alive.
    pub(crate) fn adopt_child(self: &Rc<Self>, child: &Rc<Self>) {
        if Rc::ptr_eq(self, child) {
            return;
        }
        if let Some(previous) = child
            .parent
            .borrow_mut()
            .replace(Rc::downgrade(self))
            .and_then(|parent| parent.upgrade())
        {
            previous
                .children
                .borrow_mut()
                .retain(|candidate| !Rc::ptr_eq(candidate, child));
        }
        let mut children = self.children.borrow_mut();
        if !children
            .iter()
            .any(|candidate| Rc::ptr_eq(candidate, child))
        {
            children.push(child.clone());
        }
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
        if self.destroyed.replace(true) {
            return;
        }
        self.clear_owner();
        self.retained.borrow_mut().clear();
        self.children.borrow_mut().clear();
        if let (Some(parent), Some(this)) = (
            self.parent
                .borrow_mut()
                .take()
                .and_then(|parent| parent.upgrade()),
            self.self_weak.upgrade(),
        ) {
            parent
                .children
                .borrow_mut()
                .retain(|child| !Rc::ptr_eq(child, &this));
        }
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
