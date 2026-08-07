//! Table view wrapper and its string table model.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, SelectionBridge};
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::{EventStream, ListChange, ListModel, Signal, Sink, Subscription};

pub(crate) struct TableState {
    pub(crate) table: *mut ffi::TableModel,
    list: Rc<ListModel<Vec<String>>>,
    subscription: RefCell<Option<Subscription>>,
    bindings: RefCell<Vec<Subscription>>,
}

impl Drop for TableState {
    fn drop(&mut self) {
        self.subscription.take();
        unsafe { ffi::table_model_drop(self.table) };
    }
}

fn to_rows(items: &[Vec<String>]) -> Vec<ffi::Row> {
    items
        .iter()
        .map(|cells| ffi::Row {
            cells: cells.clone(),
        })
        .collect()
}

/// A string table model that drives a Qt `QTableView` incrementally.
///
/// Rows are `Vec<String>`; changes are applied to the C++ `QAbstractTableModel`
/// with `beginInsertRows`/`beginRemoveRows`/`dataChanged`.
pub struct StringTableModel {
    pub(crate) state: Rc<TableState>,
}

impl Clone for StringTableModel {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl StringTableModel {
    /// Create a table with `columns` columns and the given header labels.
    pub fn new(columns: usize, headers: impl IntoIterator<Item = String>) -> Self {
        let table = unsafe { ffi::table_model_new(columns as i32) };
        let headers: Vec<String> = headers.into_iter().collect();
        unsafe { ffi::table_set_headers(table, headers) };
        let list = Rc::new(ListModel::new());
        let table_ptr = table;
        let subscription = list.changes().observe(move |change| match change {
            ListChange::Insert { index, items } => {
                let rows = to_rows(items);
                unsafe { ffi::table_insert_rows(table_ptr, *index as i32, rows) };
            }
            ListChange::Remove { index, len } => {
                unsafe { ffi::table_remove_rows(table_ptr, *index as i32, *len as i32) };
            }
            ListChange::Update { index, items } => {
                let rows = to_rows(items);
                unsafe { ffi::table_update_rows(table_ptr, *index as i32, rows) };
            }
            ListChange::Reset { items } => {
                let rows = to_rows(items);
                unsafe { ffi::table_reset(table_ptr, rows) };
            }
        });
        Self {
            state: Rc::new(TableState {
                table,
                list,
                subscription: RefCell::new(Some(subscription)),
                bindings: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Bind the table's rows to a signal: every emission replaces all rows.
    /// The subscription lives as long as the model.
    pub fn bind_rows(&self, signal: &Signal<Vec<Vec<String>>>) {
        let list = self.state.list.clone();
        let subscription = signal.observe(move |rows| {
            list.replace_all((*rows).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Replace the column headers.
    pub fn set_headers(&self, headers: impl IntoIterator<Item = String>) {
        unsafe {
            ffi::table_set_headers(self.state.table, headers.into_iter().collect());
        }
    }

    /// Bind the column headers to a signal.
    pub fn bind_headers(&self, signal: &Signal<Vec<String>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |headers| unsafe {
            ffi::table_set_headers(state.table, (*headers).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Set a per-row icon for column 0, given the file path whose icon should
    /// be shown (e.g. an executable). Empty paths show no icon.
    pub fn set_row_icons(&self, paths: impl IntoIterator<Item = String>) {
        unsafe { ffi::table_model_set_row_icons(self.state.table, paths.into_iter().collect()) };
    }

    /// Bind per-row icons (file paths whose icon appears in column 0) to a
    /// signal. The subscription lives as long as the model.
    pub fn bind_row_icons(&self, signal: &Signal<Vec<String>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |paths| unsafe {
            ffi::table_model_set_row_icons(state.table, (*paths).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Set normalized (0..1) heat values for `column`, rendered as a
    /// translucent intensity wash by the view's heat delegate.
    pub fn set_heat(&self, column: usize, values: impl IntoIterator<Item = f64>) {
        unsafe {
            ffi::table_model_set_heat(
                self.state.table,
                column as i32,
                values.into_iter().collect(),
            )
        };
    }

    /// Bind normalized heat values for `column` to a signal.
    pub fn bind_heat(&self, column: usize, signal: &Signal<Vec<f64>>) {
        let state = self.state.clone();
        let subscription = signal.observe(move |values| unsafe {
            ffi::table_model_set_heat(state.table, column as i32, (*values).clone());
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Bind rows and their column-0 icons from one signal of
    /// `(cells, icon path)` pairs. Rows and icons are applied atomically in a
    /// single observer, so a row reset can never wipe the same-pass icons.
    pub fn bind_table(&self, signal: &Signal<Vec<(Vec<String>, String)>>) {
        let list = self.state.list.clone();
        let state = self.state.clone();
        let subscription = signal.observe(move |rows| {
            list.replace_all(rows.iter().map(|(cells, _)| cells.clone()).collect());
            unsafe {
                ffi::table_model_set_row_icons(
                    state.table,
                    rows.iter().map(|(_, path)| path.clone()).collect(),
                );
            }
        });
        self.state.bindings.borrow_mut().push(subscription);
    }

    /// Number of rows with a non-null icon in column 0.
    pub fn row_icon_count(&self) -> usize {
        unsafe { ffi::table_model_row_icon_count(self.state.table) as usize }
    }

    /// Number of rows in the C++ mirror.
    pub fn row_count(&self) -> usize {
        unsafe { ffi::table_row_count(self.state.table) as usize }
    }

    /// Number of columns.
    pub fn column_count(&self) -> usize {
        unsafe { ffi::table_column_count(self.state.table) as usize }
    }

    /// The cell text at `(row, column)` as stored in the C++ mirror.
    pub fn cell(&self, row: usize, column: usize) -> String {
        unsafe { ffi::table_text(self.state.table, row as i32, column as i32) }
    }

    /// Append a row.
    pub fn push_row(&self, row: impl IntoIterator<Item = String>) {
        self.state.list.push(row.into_iter().collect());
    }

    /// Insert a row at `index`.
    pub fn insert_row(&self, index: usize, row: impl IntoIterator<Item = String>) {
        self.state.list.insert(index, row.into_iter().collect());
    }

    /// Remove the row at `index`.
    pub fn remove_row(&self, index: usize) -> bool {
        self.state.list.remove(index)
    }

    /// Replace the row at `index`.
    pub fn set_row(&self, index: usize, row: impl IntoIterator<Item = String>) -> bool {
        self.state.list.set(index, row.into_iter().collect())
    }

    /// Remove every row.
    pub fn clear(&self) {
        self.state.list.clear();
    }

    /// Replace the whole table.
    pub fn replace_all(&self, rows: impl IntoIterator<Item = Vec<String>>) {
        self.state.list.replace_all(rows.into_iter().collect());
    }
}

/// A `QTableView` bound to a [`StringTableModel`].
pub struct TableView {
    pub(crate) inner: Rc<Component>,
    pub(crate) table: Rc<TableState>,
    pub(crate) selection: Rc<SelectionBridge>,
}

impl Clone for TableView {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            table: self.table.clone(),
            selection: self.selection.clone(),
        }
    }
}

impl TableView {
    /// Show or hide the table without dropping its model or selection state.
    pub fn set_visible(&self, visible: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_visible(self.inner.raw(), visible) };
        }
    }

    /// Whether the table is currently visible on screen.
    pub fn is_visible(&self) -> bool {
        unsafe { ffi::widget_is_visible(self.inner.raw()) }
    }

    /// The table's current width in pixels.
    pub fn width(&self) -> i32 {
        unsafe { ffi::widget_width(self.inner.raw()) }
    }

    /// The table's current height in pixels.
    pub fn height(&self) -> i32 {
        unsafe { ffi::widget_height(self.inner.raw()) }
    }

    /// A stream of header clicks, each carrying the clicked column index.
    /// Use it to implement click-to-sort columns.
    pub fn header_clicked(&self) -> EventStream<usize> {
        if self.inner.header_sink.borrow().is_none() {
            unsafe {
                ffi::view_set_header_clicked_cb(
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

    /// Simulate a click on the column header `section` (used by tests and
    /// headless smoke runs).
    pub fn click_header(&self, section: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::view_click_header(self.inner.raw(), section as i32) };
        }
    }

    /// A stream of right-click (context menu) events on table rows, each
    /// carrying the row under the cursor.
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

    /// Simulate a right-click on `row` (used by tests and headless smoke
    /// runs); the shim shows the context menu and reports the row.
    pub fn emit_context_menu(&self, row: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::view_emit_context_menu(self.inner.raw(), row as i32) };
        }
    }

    /// Set the pixel width of `column`.
    pub fn set_column_width(&self, column: usize, width: i32) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_column_width(self.inner.raw(), column as i32, width) };
        }
    }

    /// Let the last column fill the remaining view width.
    pub fn stretch_last_section(&self, stretch: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_stretch_last_section(self.inner.raw(), stretch) };
        }
    }

    /// Select whole rows instead of individual cells.
    pub fn select_rows(&self, on: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_select_rows(self.inner.raw(), on) };
        }
    }

    /// Alternate row background colors for easier scanning.
    pub fn set_alternating_row_colors(&self, on: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_alternating_row_colors(self.inner.raw(), on) };
        }
    }

    /// Install the heat-map delegate so cells with heat values render with an
    /// intensity wash.
    pub fn enable_heat(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::view_set_heat_delegate(self.inner.raw()) };
        }
    }

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

    /// Escape hatch: the underlying Qt `QTableView` pointer.
    pub fn qobject_ptr(&self) -> *mut Void {
        self.inner.raw() as *mut Void
    }
}

impl IntoWidget for TableView {
    fn component(&self) -> &Rc<Component> {
        &self.inner
    }
}
