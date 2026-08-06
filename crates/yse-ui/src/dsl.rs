//! Declarative, tree-shaped widget construction.
//!
//! Build the UI inside [`Window::ui`]; `row`/`column` closures nest their
//! children into the layout, so the code reads top-down like a widget tree:
//!
//! ```no_run
//! use yse_model::Var;
//! use yse_ui::*;
//!
//! let app = Application::init();
//! let window = Window::new();
//! let count = Var::new(0u32);
//! window.ui(|| {
//!     column(|| {
//!         label("Count: 0").bind_text(&count.signal().map(|c| format!("Count: {c}")));
//!         row(|| {
//!             button("Increment").on_click(clone!(count => move |_| count.set(*count.value() + 1)));
//!             button("Quit");
//!         });
//!     });
//! });
//! ```
//!
//! Widgets created inside the tree are retained by their layout, so handles
//! can be dropped after construction; every binding and handler lives in the
//! component's `Owner` and is released when the widget tree is destroyed.

use crate::component::Component;
use crate::{
    Button, CheckBox, Label, LineEdit, ListView, SelectionBridge, StringListModel,
    StringTableModel, TableView, Window, ffi,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentKind {
    Window,
    Layout,
}

thread_local! {
    static PARENT: RefCell<Vec<(ParentKind, Rc<Component>)>> = const { RefCell::new(Vec::new()) };
}

fn current() -> (ParentKind, Rc<Component>) {
    PARENT.with(|stack| {
        stack
            .borrow()
            .last()
            .cloned()
            .expect("widget factories must run inside `window.ui(|| ...)`")
    })
}

fn with_parent<R, F: FnOnce() -> R>(kind: ParentKind, parent: Rc<Component>, f: F) -> R {
    PARENT.with(|stack| stack.borrow_mut().push((kind, parent)));
    let result = f();
    PARENT.with(|stack| {
        stack.borrow_mut().pop();
    });
    result
}

fn require_layout() -> Rc<Component> {
    let (kind, parent) = current();
    assert!(
        kind == ParentKind::Layout,
        "leaf widgets must live inside a row() or column(); the top of a window tree should be a layout"
    );
    parent
}

impl Window {
    /// Build the window's content as a widget tree. The closure's return value
    /// is passed through so handles can be captured; widgets created inside
    /// are retained by the tree, so dropping handles is safe.
    pub fn ui<R, F: FnOnce() -> R>(&self, f: F) -> R {
        with_parent(ParentKind::Window, self.inner.clone(), f)
    }
}

fn leaf(ptr: *mut ffi::Widget) -> Rc<Component> {
    let parent = require_layout();
    let inner = unsafe { Component::from_raw_child(ptr, &parent) };
    unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
    inner
}

/// Create a label in the current layout.
pub fn label(text: impl Into<String>) -> Label {
    let parent = require_layout();
    let inner = leaf(unsafe { ffi::widget_new_label(&text.into(), parent.raw()) });
    Label { inner }
}

/// Create a button in the current layout.
pub fn button(text: impl Into<String>) -> Button {
    let parent = require_layout();
    let inner = leaf(unsafe { ffi::widget_new_button(&text.into(), parent.raw()) });
    Button { inner }
}

/// Create a line edit in the current layout.
pub fn line_edit(text: impl Into<String>) -> LineEdit {
    let parent = require_layout();
    let inner = leaf(unsafe { ffi::widget_new_line_edit(&text.into(), parent.raw()) });
    LineEdit { inner }
}

/// Create a checkbox in the current layout.
pub fn checkbox(text: impl Into<String>) -> CheckBox {
    let parent = require_layout();
    let inner = leaf(unsafe { ffi::widget_new_checkbox(&text.into(), parent.raw()) });
    CheckBox { inner }
}

/// Nest children into a horizontal row.
pub fn row<R, F: FnOnce() -> R>(f: F) -> R {
    layout_container(ffi::widget_new_row, f)
}

/// Nest children into a vertical column.
pub fn column<R, F: FnOnce() -> R>(f: F) -> R {
    layout_container(ffi::widget_new_column, f)
}

fn layout_container<R, F: FnOnce() -> R>(
    new_container: unsafe fn(*mut ffi::Widget) -> *mut ffi::Widget,
    f: F,
) -> R {
    let (kind, parent) = current();
    let inner = unsafe { Component::from_raw_child(new_container(parent.raw()), &parent) };
    if kind == ParentKind::Layout {
        unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
    }
    with_parent(ParentKind::Layout, inner, f)
}

/// Add a stretch spacer to the current layout.
pub fn spacer() {
    let parent = require_layout();
    unsafe { ffi::layout_add_spacer(parent.raw()) };
}

/// Create a list view bound to `model` in the current layout.
pub fn list_view(model: &StringListModel) -> ListView {
    let parent = require_layout();
    let inner = unsafe {
        Component::from_raw_child(
            ffi::widget_new_list_view(model.state.model, parent.raw()),
            &parent,
        )
    };
    unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
    parent.retain(model.state.clone());
    let selection = Rc::new(SelectionBridge {
        view: inner.raw(),
        sink: RefCell::new(None),
    });
    ListView {
        inner,
        model: model.state.clone(),
        selection,
    }
}

/// Create a table view bound to `model` in the current layout.
pub fn table_view(model: &StringTableModel) -> TableView {
    let parent = require_layout();
    let inner = unsafe {
        Component::from_raw_child(
            ffi::widget_new_table_view(model.state.table, parent.raw()),
            &parent,
        )
    };
    unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
    parent.retain(model.state.clone());
    let selection = Rc::new(SelectionBridge {
        view: inner.raw(),
        sink: RefCell::new(None),
    });
    TableView {
        inner,
        table: model.state.clone(),
        selection,
    }
}
