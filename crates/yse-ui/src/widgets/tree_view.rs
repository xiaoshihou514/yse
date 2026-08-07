//! Tree model and tree view wrappers for grouped, expandable rows.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, SelectionBridge};
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink, Subscription};

pub(crate) struct TreeState {
    pub(crate) tree: *mut ffi::TreeModel,
    bindings: RefCell<Vec<Subscription>>,
}

impl Drop for TreeState {
    fn drop(&mut self) {
        unsafe { ffi::tree_model_drop(self.tree) };
    }
}

/// A tree model whose rows are supplied as flat `(parent, cells, icon)` rows;
/// group/root rows use `parent == -1`, children point at their parent's flat
/// row.
pub struct TreeModel {
    pub(crate) state: Rc<TreeState>,
}

impl Clone for TreeModel {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl TreeModel {
    /// Create a tree model with `columns` columns.
    pub fn new(columns: usize) -> Self {
        Self {
            state: Rc::new(TreeState {
                tree: unsafe { ffi::tree_model_new(columns as i32) },
                bindings: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Bind the whole tree to a signal of flat `(parent, cells, icon)` rows.
    /// The subscription lives as long as the model.
    pub fn bind(&self, signal: &Signal<Vec<(i32, Vec<String>, String)>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |rows| {
            let rows: Vec<ffi::TreeRow> = rows
                .iter()
                .map(|(parent, cells, icon)| ffi::TreeRow {
                    parent: *parent,
                    cells: cells.clone(),
                    icon: icon.clone(),
                })
                .collect();
            unsafe { ffi::tree_model_reset(state.tree, rows) };
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Bind the column headers to a signal.
    pub fn bind_headers(&self, signal: &Signal<Vec<String>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |headers| unsafe {
            ffi::tree_model_set_headers(state.tree, (*headers).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Imperatively replace the whole tree with flat `(parent, cells, icon)`
    /// rows.
    pub fn reset(&self, rows: impl IntoIterator<Item = (i32, Vec<String>, String)>) {
        let rows: Vec<ffi::TreeRow> = rows
            .into_iter()
            .map(|(parent, cells, icon)| ffi::TreeRow {
                parent,
                cells,
                icon,
            })
            .collect();
        unsafe { ffi::tree_model_reset(self.state.tree, rows) };
    }

    /// Replace the column headers.
    pub fn set_headers(&self, headers: impl IntoIterator<Item = String>) {
        unsafe {
            ffi::tree_model_set_headers(self.state.tree, headers.into_iter().collect());
        }
    }

    /// Bind normalized (0..1) heat values for `column` to a signal.
    pub fn bind_heat(&self, column: usize, signal: &Signal<Vec<f64>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |values| unsafe {
            ffi::tree_model_set_heat(state.tree, column as i32, (*values).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Number of flat rows (groups + children).
    pub fn row_count(&self) -> usize {
        unsafe { ffi::tree_model_flat_row_count(self.state.tree) as usize }
    }
}

/// A `QTreeView` bound to a [`TreeModel`].
pub struct TreeView {
    pub(crate) inner: Rc<Component>,
    pub(crate) selection: Rc<SelectionBridge>,
}

impl Clone for TreeView {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            selection: self.selection.clone(),
        }
    }
}

impl TreeView {
    /// Whether the tree is currently visible on screen.
    pub fn is_visible(&self) -> bool {
        unsafe { ffi::widget_is_visible(self.inner.raw()) }
    }

    /// The tree's current width in pixels.
    pub fn width(&self) -> i32 {
        unsafe { ffi::widget_width(self.inner.raw()) }
    }

    /// The tree's current height in pixels.
    pub fn height(&self) -> i32 {
        unsafe { ffi::widget_height(self.inner.raw()) }
    }

    /// A stream of selection changes carrying the selected flat row indices.
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

    /// Register a selection handler carrying the selected flat row indices.
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

    /// Select the flat row `row`, expanding its ancestors.
    pub fn select(&self, row: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_select_flat(self.inner.raw(), row as i32) };
        }
    }

    /// The currently selected flat row indices.
    pub fn selected_rows(&self) -> Vec<usize> {
        unsafe { ffi::view_selected_rows(self.inner.raw()) }
            .into_iter()
            .map(|row| row as usize)
            .collect()
    }

    /// A stream of header clicks carrying the clicked column index.
    pub fn header_clicked(&self) -> EventStream<usize> {
        if self.inner.header_sink.borrow().is_none() {
            unsafe {
                ffi::tree_view_set_header_clicked_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.header_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner
            .header_sink
            .borrow()
            .as_ref()
            .unwrap()
            .stream()
            .map(|section| *section as usize)
    }

    /// Simulate a header click (used by tests and smoke runs).
    pub fn click_header(&self, section: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_click_header(self.inner.raw(), section as i32) };
        }
    }

    /// A stream of right-click events carrying the flat row under the cursor.
    pub fn context_menu(&self) -> EventStream<usize> {
        if self.inner.context_sink.borrow().is_none() {
            unsafe {
                ffi::view_set_context_menu_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.context_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner
            .context_sink
            .borrow()
            .as_ref()
            .unwrap()
            .stream()
            .map(|row| *row as usize)
    }

    /// Simulate a right-click on flat row `row` (used by tests and smokes).
    pub fn emit_context_menu(&self, row: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::view_emit_context_menu(self.inner.raw(), row as i32) };
        }
    }

    /// The current vertical scroll position in pixels.
    pub fn scroll_value(&self) -> i32 {
        unsafe { ffi::tree_view_scroll_value(self.inner.raw()) }
    }

    /// Restore the vertical scroll position in pixels.
    pub fn set_scroll_value(&self, value: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_set_scroll_value(self.inner.raw(), value) };
        }
    }

    /// Expand or collapse all group rows.
    pub fn expand_all(&self, expand: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_expand_all(self.inner.raw(), expand) };
        }
    }

    /// Flat rows that are currently expanded.
    pub fn expanded_rows(&self) -> Vec<usize> {
        unsafe { ffi::tree_view_expanded_rows(self.inner.raw()) }
            .into_iter()
            .map(|row| row as usize)
            .collect()
    }

    /// Expand the given flat rows.
    pub fn expand_rows(&self, rows: &[usize]) {
        if self.inner.is_alive() {
            unsafe {
                ffi::tree_view_expand_rows(
                    self.inner.raw(),
                    rows.iter().map(|row| *row as i32).collect(),
                )
            };
        }
    }

    /// The flat row currently scrolled to the top of the viewport, if any.
    pub fn top_row(&self) -> Option<usize> {
        let row = unsafe { ffi::tree_view_top_row(self.inner.raw()) };
        (row >= 0).then_some(row as usize)
    }

    /// Scroll the viewport so `flat` is at the top.
    pub fn scroll_to_flat(&self, flat: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_scroll_to_flat(self.inner.raw(), flat as i32) };
        }
    }

    /// Set the pixel width of `column`.
    pub fn set_column_width(&self, column: usize, width: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_set_column_width(self.inner.raw(), column as i32, width) };
        }
    }

    /// Hide or show `column` without changing the model layout.
    pub fn set_column_hidden(&self, column: usize, hidden: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_column_hidden(self.inner.raw(), column as i32, hidden) };
        }
    }

    /// Let the last column fill the remaining width.
    pub fn stretch_last_section(&self, stretch: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::tree_view_stretch_last_section(self.inner.raw(), stretch) };
        }
    }

    /// Select whole rows instead of individual cells.
    pub fn select_rows(&self, on: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_select_rows(self.inner.raw(), on) };
        }
    }

    /// Alternate row background colors.
    pub fn set_alternating_row_colors(&self, on: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_alternating_row_colors(self.inner.raw(), on) };
        }
    }

    /// Install the heat-map delegate.
    pub fn enable_heat(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_heat_delegate(self.inner.raw()) };
        }
    }
}

impl IntoWidget for TreeView {
    fn component(&self) -> &Rc<Component> {
        &self.inner
    }
}
