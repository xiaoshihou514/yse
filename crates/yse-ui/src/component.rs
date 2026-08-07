//! Internal component state shared by every widget wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::thread::ThreadId;
use yse_model::{Owner, Sink};

pub(crate) type RetainedId = u64;

/// Log an error when a C++-side callback arrives on a different thread than
/// the one that created the object. Qt delivers widget signals and queued
/// invocations on the GUI thread, so a mismatch means the object was used
/// from a worker thread — a misuse Yse does not support.
pub(crate) fn log_off_thread(what: &str) {
    let message = format!("{what} invoked off the thread that created it");
    yse_model::log_error(message.clone());
    eprintln!("yse: {message}");
}

/// Internal component state shared by every widget wrapper.
#[doc(hidden)]
pub struct Component {
    pub ptr: *mut ffi::Widget,
    pub owner: RefCell<Owner>,
    pub destroyed: Cell<bool>,
    created_on: ThreadId,
    parent: RefCell<Option<Weak<Component>>>,
    self_weak: Weak<Component>,
    children: RefCell<Vec<Rc<Component>>>,
    retained: RefCell<HashMap<RetainedId, Rc<dyn Any>>>,
    next_retained_id: Cell<RetainedId>,
    pub updating: Cell<bool>,
    pub clicked_sink: RefCell<Option<Rc<Sink<()>>>>,
    pub text_sink: RefCell<Option<Rc<Sink<String>>>>,
    pub toggled_sink: RefCell<Option<Rc<Sink<bool>>>>,
    pub value_sink: RefCell<Option<Rc<Sink<i32>>>>,
    pub date_sink: RefCell<Option<Rc<Sink<String>>>>,
    pub header_sink: RefCell<Option<Rc<Sink<i32>>>>,
    pub context_sink: RefCell<Option<Rc<Sink<i32>>>>,
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
            created_on: std::thread::current().id(),
            parent: RefCell::new(parent.map(Rc::downgrade)),
            self_weak: self_weak.clone(),
            children: RefCell::new(Vec::new()),
            retained: RefCell::new(HashMap::new()),
            next_retained_id: Cell::new(0),
            updating: Cell::new(false),
            clicked_sink: RefCell::new(None),
            text_sink: RefCell::new(None),
            toggled_sink: RefCell::new(None),
            value_sink: RefCell::new(None),
            date_sink: RefCell::new(None),
            header_sink: RefCell::new(None),
            context_sink: RefCell::new(None),
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
        self.check_thread("widget clicked");
        if let Some(sink) = self.clicked_sink.borrow().as_ref() {
            sink.send(());
        }
    }

    pub fn on_text_changed(&self) {
        if !self.is_alive() || self.updating.get() {
            return;
        }
        self.check_thread("text changed");
        let text = unsafe { ffi::line_edit_text(self.ptr) };
        if let Some(sink) = self.text_sink.borrow().as_ref() {
            sink.send(text);
        }
    }

    pub fn on_toggled(&self) {
        if !self.is_alive() || self.updating.get() {
            return;
        }
        self.check_thread("toggled");
        let checked = unsafe { ffi::checkbox_checked(self.ptr) };
        if let Some(sink) = self.toggled_sink.borrow().as_ref() {
            sink.send(checked);
        }
    }

    pub fn on_value_changed(&self) {
        if !self.is_alive() {
            return;
        }
        self.check_thread("value changed");
        let value = unsafe { ffi::widget_value(self.ptr) };
        if let Some(sink) = self.value_sink.borrow().as_ref() {
            sink.send(value);
        }
    }

    pub fn on_date_value_changed(&self) {
        if !self.is_alive() {
            return;
        }
        self.check_thread("date value changed");
        let value = unsafe { ffi::widget_value_text(self.ptr) };
        if let Some(sink) = self.date_sink.borrow().as_ref() {
            sink.send(value);
        }
    }

    pub fn on_header_clicked(&self, section: i32) {
        if !self.is_alive() {
            return;
        }
        self.check_thread("header clicked");
        if let Some(sink) = self.header_sink.borrow().as_ref() {
            sink.send(section);
        }
    }

    pub fn on_view_context_menu(&self, row: i32) {
        if !self.is_alive() {
            return;
        }
        self.check_thread("view context menu");
        if let Some(sink) = self.context_sink.borrow().as_ref() {
            sink.send(row);
        }
    }

    pub fn on_destroyed(&self) {
        if self.destroyed.replace(true) {
            return;
        }
        self.check_thread("widget destroyed");
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

    fn check_thread(&self, what: &str) {
        if std::thread::current().id() != self.created_on {
            log_off_thread(what);
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

    pub fn set_widget_value(&self, value: i32) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::widget_set_value(self.ptr, value) };
    }

    pub fn set_widget_visible(&self, visible: bool) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::widget_set_visible(self.ptr, visible) };
    }

    pub fn set_button_text(&self, text: String) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::button_set_text(self.ptr, &text) };
    }

    pub fn set_chart_series(&self, points: Vec<f64>) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::line_chart_set_series(self.ptr, points) };
    }

    pub fn set_chart_series_multi(&self, points: Vec<f64>, series: usize) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::line_chart_set_series_multi(self.ptr, points, series) };
    }

    pub fn set_date_value(&self, text: String) {
        if !self.is_alive() {
            return;
        }
        unsafe { ffi::widget_set_value_text(self.ptr, &text) };
    }
}

impl Drop for Component {
    fn drop(&mut self) {
        unsafe { ffi::widget_drop(self.ptr) };
    }
}
