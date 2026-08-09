//! Row / column / grid layout containers.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{
    Button, CheckBox, ComboBox, DateEdit, DateTimeEdit, IntoWidget, Label, LineChart, LineEdit,
    ListView, ProgressBar, SelectionBridge, Slider, SpinBox, StringListModel, StringTableModel,
    TabWidget, TableView, TimeEdit, TreeModel, TreeView, widget_wrapper,
};
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Signal;

/// A retained horizontal box layout.
pub struct Row {
    pub(crate) inner: Rc<Component>,
}

impl Row {
    pub(crate) fn label_raw(&self, text: impl Into<String>) -> Rc<Component> {
        unsafe {
            Component::from_raw_child(
                ffi::widget_new_label(&text.into(), self.inner.raw()),
                &self.inner,
            )
        }
    }

    /// Create a label in this row.
    pub fn label(&self, text: impl Into<String>) -> Label {
        Label {
            inner: self.label_raw(text),
        }
    }

    /// Create a button in this row.
    pub fn button(&self, text: impl Into<String>) -> Button {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_button(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Button { inner }
    }

    /// Create a line edit in this row.
    pub fn line_edit(&self, text: impl Into<String>) -> LineEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_line_edit(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        LineEdit { inner }
    }

    /// Create a native Qt date/time editor with a calendar popup.
    pub fn date_time_edit(&self, iso_datetime: impl Into<String>) -> DateTimeEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_datetime_edit(&iso_datetime.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        DateTimeEdit { inner }
    }

    /// Create a native Qt calendar date editor.
    pub fn date_edit(&self, iso_date: impl Into<String>) -> DateEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_date_edit(&iso_date.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        DateEdit { inner }
    }

    /// Create a native Qt time editor with hour and minute spin controls.
    pub fn time_edit(&self, iso_time: impl Into<String>) -> TimeEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_time_edit(&iso_time.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        TimeEdit { inner }
    }

    /// Create a checkbox in this row.
    pub fn checkbox(&self, text: impl Into<String>) -> CheckBox {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_checkbox(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        CheckBox { inner }
    }

    /// Create a combo box with `items` in this row.
    pub fn combo_box<I, S>(&self, items: I) -> ComboBox
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_combo(
                    items.into_iter().map(Into::into).collect(),
                    self.inner.raw(),
                ),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        ComboBox { inner }
    }

    /// Create a numeric spinner (default range `0..=100`) in this row.
    pub fn spin_box(&self, value: i32) -> SpinBox {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_spin_box(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        SpinBox { inner }
    }

    /// Create a horizontal slider (default range `0..=100`) in this row.
    pub fn slider(&self, value: i32) -> Slider {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_slider(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Slider { inner }
    }

    /// Create a progress bar (default range `0..=100`) in this row.
    pub fn progress_bar(&self, value: i32) -> ProgressBar {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_progress_bar(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        ProgressBar { inner }
    }

    /// Create a tabbed container in this row.
    pub fn tab_widget(&self) -> TabWidget {
        let inner = unsafe {
            Component::from_raw_child(ffi::widget_new_tab_widget(self.inner.raw()), &self.inner)
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        TabWidget { inner }
    }

    /// Create a painted line chart in this row.
    pub fn line_chart(&self) -> LineChart {
        let inner = unsafe {
            Component::from_raw_child(ffi::widget_new_line_chart(self.inner.raw()), &self.inner)
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        LineChart { inner }
    }

    /// Create a list view bound to `model`.
    pub fn list_view(&self, model: &StringListModel) -> ListView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_list_view(model.state.model, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        ListView {
            inner,
            model: model.state.clone(),
            selection,
        }
    }

    /// Create a table view bound to `model`.
    pub fn table_view(&self, model: &StringTableModel) -> TableView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_table_view(model.state.table, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TableView {
            inner,
            table: model.state.clone(),
            selection,
        }
    }

    /// Create a tree view bound to `model`.
    pub fn tree_view(&self, model: &TreeModel) -> TreeView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_tree_view(model.state.tree, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TreeView { inner, selection }
    }

    /// Add a stretch spacer at the end of the row.
    pub fn spacer(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::layout_add_spacer(self.inner.raw()) };
        }
    }

    /// Add an existing widget to the row.
    pub fn add(&self, widget: &impl IntoWidget) {
        if self.inner.is_alive() {
            unsafe { ffi::layout_add(self.inner.raw(), widget.component().raw()) };
            self.inner.adopt_child(widget.component());
        }
    }
}

widget_wrapper!(Row);

/// A vertical column of widgets.
pub struct Column {
    pub(crate) inner: Rc<Component>,
}

impl Column {
    /// Create a label in this column.
    pub fn label(&self, text: impl Into<String>) -> Label {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_label(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Label { inner }
    }

    /// Create a button in this column.
    pub fn button(&self, text: impl Into<String>) -> Button {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_button(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Button { inner }
    }

    /// Create a line edit in this column.
    pub fn line_edit(&self, text: impl Into<String>) -> LineEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_line_edit(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        LineEdit { inner }
    }

    /// Create a checkbox in this column.
    pub fn checkbox(&self, text: impl Into<String>) -> CheckBox {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_checkbox(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        CheckBox { inner }
    }

    /// Create a combo box with `items` in this column.
    pub fn combo_box<I, S>(&self, items: I) -> ComboBox
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_combo(
                    items.into_iter().map(Into::into).collect(),
                    self.inner.raw(),
                ),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        ComboBox { inner }
    }

    /// Create a numeric spinner (default range `0..=100`) in this column.
    pub fn spin_box(&self, value: i32) -> SpinBox {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_spin_box(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        SpinBox { inner }
    }

    /// Create a horizontal slider (default range `0..=100`) in this column.
    pub fn slider(&self, value: i32) -> Slider {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_slider(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Slider { inner }
    }

    /// Create a progress bar (default range `0..=100`) in this column.
    pub fn progress_bar(&self, value: i32) -> ProgressBar {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_progress_bar(0, 100, value, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        ProgressBar { inner }
    }

    /// Create a list view bound to `model`.
    pub fn list_view(&self, model: &StringListModel) -> ListView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_list_view(model.state.model, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        ListView {
            inner,
            model: model.state.clone(),
            selection,
        }
    }

    /// Create a table view bound to `model`.
    pub fn table_view(&self, model: &StringTableModel) -> TableView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_table_view(model.state.table, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TableView {
            inner,
            table: model.state.clone(),
            selection,
        }
    }

    /// Create a tree view bound to `model`.
    pub fn tree_view(&self, model: &TreeModel) -> TreeView {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_tree_view(model.state.tree, self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TreeView { inner, selection }
    }

    /// Add a stretch spacer at the end of the column.
    pub fn spacer(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::layout_add_spacer(self.inner.raw()) };
        }
    }

    /// Add an existing widget to the column.
    pub fn add(&self, widget: &impl IntoWidget) {
        if self.inner.is_alive() {
            unsafe { ffi::layout_add(self.inner.raw(), widget.component().raw()) };
            self.inner.adopt_child(widget.component());
        }
    }

    /// Create a tabbed container in this column.
    pub fn tab_widget(&self) -> TabWidget {
        let inner = unsafe {
            Component::from_raw_child(ffi::widget_new_tab_widget(self.inner.raw()), &self.inner)
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        TabWidget { inner }
    }

    /// Create a painted line chart in this column.
    pub fn line_chart(&self) -> LineChart {
        let inner = unsafe {
            Component::from_raw_child(ffi::widget_new_line_chart(self.inner.raw()), &self.inner)
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        LineChart { inner }
    }

    /// Create a native Qt date/time editor with a calendar popup.
    pub fn date_time_edit(&self, iso_datetime: impl Into<String>) -> DateTimeEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_datetime_edit(&iso_datetime.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        DateTimeEdit { inner }
    }

    /// Create a native Qt calendar date editor.
    pub fn date_edit(&self, iso_date: impl Into<String>) -> DateEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_date_edit(&iso_date.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        DateEdit { inner }
    }

    /// Create a native Qt time editor with hour and minute spin controls.
    pub fn time_edit(&self, iso_time: impl Into<String>) -> TimeEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_time_edit(&iso_time.into(), self.inner.raw()),
                &self.inner,
            )
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        TimeEdit { inner }
    }
}

widget_wrapper!(Column);

/// A grid layout.
pub struct Grid {
    pub(crate) inner: Rc<Component>,
}

impl Grid {
    /// Create a label at `(row, column)`.
    pub fn label(&self, text: impl Into<String>, row: u32, column: u32) -> Label {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_label(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        self.add(
            &Label {
                inner: inner.clone(),
            },
            row,
            column,
        );
        Label { inner }
    }

    /// Create a button at `(row, column)`.
    pub fn button(&self, text: impl Into<String>, row: u32, column: u32) -> Button {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_button(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        self.add(
            &Button {
                inner: inner.clone(),
            },
            row,
            column,
        );
        Button { inner }
    }

    /// Create a line edit at `(row, column)`.
    pub fn line_edit(&self, text: impl Into<String>, row: u32, column: u32) -> LineEdit {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_line_edit(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        self.add(
            &LineEdit {
                inner: inner.clone(),
            },
            row,
            column,
        );
        LineEdit { inner }
    }

    /// Create a checkbox at `(row, column)`.
    pub fn checkbox(&self, text: impl Into<String>, row: u32, column: u32) -> CheckBox {
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_checkbox(&text.into(), self.inner.raw()),
                &self.inner,
            )
        };
        self.add(
            &CheckBox {
                inner: inner.clone(),
            },
            row,
            column,
        );
        CheckBox { inner }
    }

    /// Add an existing widget at `(row, column)`.
    pub fn add(&self, widget: &impl IntoWidget, row: u32, column: u32) {
        if self.inner.is_alive() {
            unsafe {
                ffi::grid_add(
                    self.inner.raw(),
                    widget.component().raw(),
                    row as i32,
                    column as i32,
                );
            };
            self.inner.adopt_child(widget.component());
        }
    }
}

widget_wrapper!(Grid);
