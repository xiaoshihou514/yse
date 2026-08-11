//! List view wrapper and its string list model.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, SelectionBridge};
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::{EventStream, ListChange, ListModel, Sink, Subscription};

/// A retained list view backed by a [`StringListModel`].
pub struct ListView {
    pub(crate) inner: Rc<Component>,
    pub(crate) model: Rc<ModelState>,
    pub(crate) selection: Rc<SelectionBridge>,
}

impl Clone for ListView {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            model: self.model.clone(),
            selection: self.selection.clone(),
        }
    }
}

impl ListView {
    /// A stream of selection changes, each carrying the selected row indices.
    pub fn selection_changed(&self) -> EventStream<Vec<usize>> {
        if self.selection.sink.borrow().is_none() {
            unsafe {
                ffi::view_set_selection_cb(
                    self.inner.raw(),
                    &*self.selection as *const SelectionBridge as *mut Void,
                );
            }
            *self.selection.sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.selection.sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a selection handler carrying the selected row indices.
    pub fn on_selection<F>(&self, f: F) -> &Self
    where
        F: FnMut(&Vec<usize>) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.selection_changed().observe(f));
        self
    }

    /// Select `row`, replacing the current selection.
    pub fn select(&self, row: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::view_select_row(self.inner.raw(), row as i32) };
        }
    }

    /// Clear the selection.
    pub fn clear_selection(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::view_clear_selection(self.inner.raw()) };
        }
    }

    /// The currently selected row indices.
    pub fn selected_rows(&self) -> Vec<usize> {
        unsafe { ffi::view_selected_rows(self.inner.raw()) }
            .into_iter()
            .map(|row| row as usize)
            .collect()
    }

    /// Escape hatch: the underlying Qt `QListView` pointer.
    ///
    /// The pointer is valid only while this view's Qt object is alive. Do not
    /// retain it after destruction or use it from a thread other than the GUI
    /// thread. Dereferencing or passing the pointer across FFI remains unsafe.
    pub fn qobject_ptr(&self) -> *mut Void {
        self.inner.raw() as *mut Void
    }
}

impl IntoWidget for ListView {
    fn component(&self) -> &Rc<Component> {
        &self.inner
    }
}

pub(crate) struct ModelState {
    pub(crate) model: *mut ffi::Model,
    list: Rc<ListModel<String>>,
    subscription: RefCell<Option<Subscription>>,
}

impl Drop for ModelState {
    fn drop(&mut self) {
        // Unsubscribe before freeing the C++ model so no change event can
        // touch a freed pointer.
        self.subscription.take();
        unsafe { ffi::model_drop(self.model) };
    }
}

/// A string list model that drives a Qt `QListView` incrementally.
///
/// Mutations update a [`yse_model::ListModel`] and are applied to the C++
/// `QAbstractListModel` through `beginInsertRows`/`beginRemoveRows`/
/// `dataChanged`, so the view never resets for ordinary edits.
pub struct StringListModel {
    pub(crate) state: Rc<ModelState>,
}

impl Clone for StringListModel {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl StringListModel {
    /// Create an empty model.
    pub fn new() -> Self {
        let model = unsafe { ffi::model_new() };
        let list = Rc::new(ListModel::new());
        let model_ptr = model;
        let subscription = list.changes().observe(move |change| match change {
            ListChange::Insert { index, items } => {
                unsafe { ffi::model_insert_rows(model_ptr, *index as i32, items.clone()) };
            }
            ListChange::Remove { index, len } => {
                unsafe { ffi::model_remove_rows(model_ptr, *index as i32, *len as i32) };
            }
            ListChange::Update { index, items } => {
                unsafe { ffi::model_update_rows(model_ptr, *index as i32, items.clone()) };
            }
            ListChange::Reset { items } => {
                unsafe { ffi::model_reset(model_ptr, items.clone()) };
            }
        });
        Self {
            state: Rc::new(ModelState {
                model,
                list,
                subscription: RefCell::new(Some(subscription)),
            }),
        }
    }

    /// Number of rows in the C++ mirror (equal to the list length).
    pub fn row_count(&self) -> usize {
        unsafe { ffi::model_row_count(self.state.model) as usize }
    }

    /// The text of a row as stored in the C++ mirror.
    pub fn row_text(&self, row: usize) -> String {
        unsafe { ffi::model_text(self.state.model, row as i32) }
    }

    /// The underlying pure-Rust list.
    pub fn list(&self) -> &Rc<ListModel<String>> {
        &self.state.list
    }

    /// Append `item`.
    pub fn push(&self, item: impl Into<String>) {
        self.state.list.push(item.into());
    }

    /// Append every item in `items`.
    pub fn extend(&self, items: impl IntoIterator<Item = String>) {
        self.state.list.extend(items.into_iter().collect());
    }

    /// Insert `item` at `index`.
    pub fn insert(&self, index: usize, item: impl Into<String>) {
        self.state.list.insert(index, item.into());
    }

    /// Remove the row at `index`.
    pub fn remove(&self, index: usize) -> bool {
        self.state.list.remove(index)
    }

    /// Replace the row at `index`.
    pub fn set(&self, index: usize, item: impl Into<String>) -> bool {
        self.state.list.set(index, item.into())
    }

    /// Remove every row.
    pub fn clear(&self) {
        self.state.list.clear();
    }

    /// Replace the whole list.
    pub fn replace_all(&self, items: impl IntoIterator<Item = String>) {
        self.state.list.replace_all(items.into_iter().collect());
    }
}

impl Default for StringListModel {
    fn default() -> Self {
        Self::new()
    }
}
