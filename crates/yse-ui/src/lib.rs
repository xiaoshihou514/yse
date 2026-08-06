//! `yse-ui` binds [`yse_model`] to a retained Qt Widgets tree.
//!
//! Components are plain Rust wrappers around a small hand-written C++ shim.
//! Every component owns its subscriptions through an `Owner` that is tied to
//! the Qt object lifetime: when the widget is destroyed, all of its bindings
//! are released automatically.

mod action;
mod callback;
mod component;
mod dsl;
mod media;

#[cxx::bridge(namespace = "yse_ui")]
mod bridge {
    /// A table row, passed to the C++ shim as a flat string list.
    struct Row {
        cells: Vec<String>,
    }

    struct MediaProbeResult {
        ok: bool,
        summary: String,
        duration_ms: i64,
        has_video: bool,
        has_audio: bool,
    }

    struct MediaConvertResult {
        ok: bool,
        message: String,
    }

    unsafe extern "C++" {
        include!("yse-ui/src/widgets.h");
        include!("yse-ui/src/media.h");

        type Widget;
        type Void;
        type Action;
        type Model;
        type TableModel;
        type Dialog;
        type FileDialog;
        type Settings;

        fn app_init();
        fn app_exec() -> i32;
        fn app_quit_after(ms: i32);
        unsafe fn app_schedule_gui(task: *mut Void);
        unsafe fn app_invoke_after(ms: i32, task: *mut Void);

        unsafe fn widget_new_window() -> *mut Widget;
        unsafe fn widget_new_label(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_button(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_line_edit(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_checkbox(text: &str, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_row(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_column(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_grid(parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_menubar(window: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_toolbar(window: *mut Widget) -> *mut Widget;
        unsafe fn menu_new(title: &str, menubar: *mut Widget) -> *mut Widget;
        unsafe fn menu_add_separator(menu: *mut Widget);
        unsafe fn toolbar_add_action(toolbar: *mut Widget, action: *mut Action);
        unsafe fn widget_drop(w: *mut Widget);
        unsafe fn widget_show(w: *mut Widget);
        unsafe fn widget_set_visible(w: *mut Widget, visible: bool);
        unsafe fn widget_set_enabled(w: *mut Widget, enabled: bool);
        unsafe fn widget_set_title(w: *mut Widget, title: &str);
        unsafe fn widget_resize(w: *mut Widget, width: i32, height: i32);
        unsafe fn layout_add(layout: *mut Widget, child: *mut Widget);
        unsafe fn layout_add_spacer(w: *mut Widget);
        unsafe fn grid_add(grid: *mut Widget, child: *mut Widget, row: i32, column: i32);
        unsafe fn label_set_text(w: *mut Widget, text: &str);
        unsafe fn label_text(w: *mut Widget) -> String;
        unsafe fn line_edit_set_text(w: *mut Widget, text: &str);
        unsafe fn line_edit_text(w: *mut Widget) -> String;
        unsafe fn checkbox_set_checked(w: *mut Widget, checked: bool);
        unsafe fn checkbox_checked(w: *mut Widget) -> bool;
        unsafe fn button_click(w: *mut Widget);
        unsafe fn action_new(text: &str, parent: *mut Widget) -> *mut Action;
        unsafe fn action_drop(a: *mut Action);
        unsafe fn action_set_text(a: *mut Action, text: &str);
        unsafe fn action_set_enabled(a: *mut Action, enabled: bool);
        unsafe fn action_set_shortcut(a: *mut Action, shortcut: &str);
        unsafe fn action_trigger(a: *mut Action);
        unsafe fn action_set_triggered_cb(a: *mut Action, data: *mut Void);
        unsafe fn action_set_destroyed_cb(a: *mut Action, data: *mut Void);
        unsafe fn model_new() -> *mut Model;
        unsafe fn model_drop(m: *mut Model);
        unsafe fn model_insert_rows(m: *mut Model, row: i32, items: Vec<String>);
        unsafe fn model_remove_rows(m: *mut Model, row: i32, count: i32);
        unsafe fn model_update_rows(m: *mut Model, row: i32, items: Vec<String>);
        unsafe fn model_reset(m: *mut Model, items: Vec<String>);
        unsafe fn model_row_count(m: *mut Model) -> i32;
        unsafe fn model_text(m: *mut Model, row: i32) -> String;
        unsafe fn widget_new_list_view(model: *mut Model, parent: *mut Widget) -> *mut Widget;
        unsafe fn widget_new_table_view(model: *mut TableModel, parent: *mut Widget)
        -> *mut Widget;
        unsafe fn table_model_new(columns: i32) -> *mut TableModel;
        unsafe fn table_model_drop(m: *mut TableModel);
        unsafe fn table_set_headers(m: *mut TableModel, headers: Vec<String>);
        unsafe fn table_insert_rows(m: *mut TableModel, row: i32, rows: Vec<Row>);
        unsafe fn table_remove_rows(m: *mut TableModel, row: i32, count: i32);
        unsafe fn table_update_rows(m: *mut TableModel, row: i32, rows: Vec<Row>);
        unsafe fn table_reset(m: *mut TableModel, rows: Vec<Row>);
        unsafe fn table_row_count(m: *mut TableModel) -> i32;
        unsafe fn table_column_count(m: *mut TableModel) -> i32;
        unsafe fn table_text(m: *mut TableModel, row: i32, column: i32) -> String;
        unsafe fn view_set_selection_cb(view: *mut Widget, data: *mut Void);
        unsafe fn view_selected_rows(view: *mut Widget) -> Vec<i32>;
        unsafe fn view_select_row(view: *mut Widget, row: i32);
        unsafe fn view_clear_selection(view: *mut Widget);
        unsafe fn dialog_message_new(
            parent: *mut Widget,
            title: &str,
            text: &str,
            buttons: i32,
        ) -> *mut Dialog;
        unsafe fn dialog_show(d: *mut Dialog);
        unsafe fn dialog_accept(d: *mut Dialog);
        unsafe fn dialog_close(d: *mut Dialog);
        unsafe fn dialog_last_result(d: *mut Dialog) -> i32;
        unsafe fn dialog_set_finished_cb(d: *mut Dialog, data: *mut Void);
        unsafe fn dialog_drop(d: *mut Dialog);
        unsafe fn filedialog_open_new(parent: *mut Widget, title: &str) -> *mut FileDialog;
        unsafe fn filedialog_show(d: *mut FileDialog);
        unsafe fn filedialog_accept(d: *mut FileDialog);
        unsafe fn filedialog_close(d: *mut FileDialog);
        unsafe fn filedialog_selected_file(d: *mut FileDialog) -> String;
        unsafe fn filedialog_set_finished_cb(d: *mut FileDialog, data: *mut Void);
        unsafe fn filedialog_drop(d: *mut FileDialog);
        unsafe fn settings_new(organization: &str, application: &str) -> *mut Settings;
        unsafe fn settings_drop(s: *mut Settings);
        unsafe fn settings_value(s: *mut Settings, key: &str) -> String;
        unsafe fn settings_set(s: *mut Settings, key: &str, value: &str);
        unsafe fn settings_remove(s: *mut Settings, key: &str);
        unsafe fn settings_contains(s: *mut Settings, key: &str) -> bool;
        unsafe fn settings_sync(s: *mut Settings);
        unsafe fn widget_set_destroyed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_clicked_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_text_changed_cb(w: *mut Widget, data: *mut Void);
        unsafe fn widget_set_toggled_cb(w: *mut Widget, data: *mut Void);

        fn media_probe(path: &str) -> MediaProbeResult;
        fn media_convert(input: &str, output: &str, preset: i32) -> MediaConvertResult;
    }

    extern "Rust" {
        unsafe fn on_widget_clicked(data: *mut Void);
        unsafe fn on_text_changed(data: *mut Void);
        unsafe fn on_toggled(data: *mut Void);
        unsafe fn on_widget_destroyed(data: *mut Void);
        unsafe fn on_gui_scheduled(task: *mut Void);
        unsafe fn on_app_timer(task: *mut Void);
        unsafe fn on_action_triggered(data: *mut Void);
        unsafe fn on_action_destroyed(data: *mut Void);
        unsafe fn on_selection_changed(data: *mut Void);
        unsafe fn on_dialog_finished(data: *mut Void);
        unsafe fn on_filedialog_finished(data: *mut Void);
    }
}

unsafe fn on_widget_clicked(data: *mut bridge::Void) {
    unsafe { callback::clicked(data) };
}

unsafe fn on_text_changed(data: *mut bridge::Void) {
    unsafe { callback::text_changed(data) };
}

unsafe fn on_toggled(data: *mut bridge::Void) {
    unsafe { callback::toggled(data) };
}

unsafe fn on_widget_destroyed(data: *mut bridge::Void) {
    unsafe { callback::destroyed(data) };
}

unsafe fn on_gui_scheduled(task: *mut bridge::Void) {
    unsafe {
        let task: Box<Box<dyn FnOnce() + Send>> =
            Box::from_raw(task as *mut Box<dyn FnOnce() + Send>);
        task();
    }
}

unsafe fn on_app_timer(task: *mut bridge::Void) {
    unsafe {
        let task: Box<Box<dyn FnOnce()>> = Box::from_raw(task as *mut Box<dyn FnOnce()>);
        task();
    }
}

unsafe fn on_action_triggered(data: *mut bridge::Void) {
    unsafe { callback::action_triggered(data) };
}

unsafe fn on_action_destroyed(data: *mut bridge::Void) {
    unsafe { callback::action_destroyed(data) };
}

unsafe fn on_selection_changed(data: *mut bridge::Void) {
    unsafe { callback::selection_changed(data) };
}

unsafe fn on_dialog_finished(data: *mut bridge::Void) {
    unsafe { callback::dialog_finished(data) };
}

unsafe fn on_filedialog_finished(data: *mut bridge::Void) {
    unsafe { callback::filedialog_finished(data) };
}

use crate::action::ActionState;
use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::{Component, RetainedId};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use yse_model::{EventStream, ListChange, ListModel, Scheduler, Signal, Sink, Subscription, Var};

pub use dsl::{
    BoolValue, ButtonView, CheckBoxView, IntoBoolValue, IntoTextValue, LabelView, LayoutView,
    LineEditView, MountContext, SpacerView, TextValue, Ui, View, button, checkbox, column, label,
    line_edit, row, spacer,
};
pub use media::{MediaInfo, MediaPreset, convert_media, probe_media};

/// Clone each named binding and move the clones into `body` (typically a
/// `move` closure), avoiding `let x = x.clone();` boilerplate.
#[macro_export]
macro_rules! clone {
    ($($name:ident),+ => $body:expr) => {
        {
            $(let $name = $name.clone();)+
            $body
        }
    };
}

/// The Qt application object. Must be created before any window.
#[derive(Clone, Copy)]
pub struct Application;

impl Application {
    /// Initialise the Qt application (idempotent).
    pub fn init() -> Self {
        ffi::app_init();
        Self
    }

    /// Run the Qt event loop; returns the application exit code.
    pub fn exec(&self) -> i32 {
        ffi::app_exec()
    }

    /// Schedule `QCoreApplication::quit` after `ms` milliseconds.
    pub fn quit_after(&self, ms: u64) {
        ffi::app_quit_after(ms as i32);
    }

    /// Run `f` on the GUI thread after `ms` milliseconds. If the app quits
    /// first, the closure is leaked (one-shot timer delivery).
    pub fn after<F>(&self, ms: u64, f: F)
    where
        F: FnOnce() + 'static,
    {
        let outer: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
        let ptr = Box::into_raw(outer) as *mut Void;
        unsafe { ffi::app_invoke_after(ms as i32, ptr) };
    }
}

/// A [`Scheduler`] that runs tasks on the Qt event loop, i.e. the GUI thread.
/// Use this as the delivery target for [`yse_model::spawn_task`].
pub struct QtGuiScheduler;

impl Scheduler for QtGuiScheduler {
    fn schedule(&self, task: Box<dyn FnOnce() + Send>) {
        // The closure is leaked to a thin pointer that Qt hands back on the
        // GUI thread, where on_gui_scheduled reconstructs and runs it. If the
        // app quits before the queued call runs, the closure leaks; this is
        // acceptable for one-shot deliveries.
        let outer: Box<Box<dyn FnOnce() + Send>> = Box::new(task);
        let ptr = Box::into_raw(outer) as *mut Void;
        unsafe { ffi::app_schedule_gui(ptr) };
    }
}

/// A top-level window.
pub struct Window {
    inner: Rc<Component>,
}

impl Window {
    /// Create a new window.
    pub fn new() -> Self {
        Self {
            inner: unsafe { Component::from_raw(ffi::widget_new_window()) },
        }
    }

    /// Show the window.
    pub fn show(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_show(self.inner.raw()) };
        }
    }

    /// Show or hide the window.
    pub fn set_visible(&self, visible: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_visible(self.inner.raw(), visible) };
        }
    }

    /// Set the window title.
    pub fn set_title(&self, title: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_title(self.inner.raw(), &title.into()) };
        }
    }

    /// Set the window size in pixels.
    pub fn set_size(&self, width: u32, height: u32) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_resize(self.inner.raw(), width as i32, height as i32) };
        }
    }

    /// Create a horizontal row inside this window.
    pub fn row(&self) -> Row {
        Row {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_row(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a vertical column inside this window.
    pub fn column(&self) -> Column {
        Column {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_column(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a grid layout inside this window.
    pub fn grid(&self) -> Grid {
        Grid {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_grid(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a menu bar for this window.
    pub fn menu_bar(&self) -> MenuBar {
        MenuBar {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_menubar(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a toolbar for this window.
    pub fn toolbar(&self) -> ToolBar {
        ToolBar {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_toolbar(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Escape hatch: the underlying Qt `QWidget` pointer.
    pub fn qobject_ptr(&self) -> *mut Void {
        self.inner.raw() as *mut Void
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// Widgets that can be placed into a layout.
#[doc(hidden)]
pub trait IntoWidget {
    /// Access the underlying component.
    #[doc(hidden)]
    fn component(&self) -> &Rc<Component>;
}

macro_rules! widget_wrapper {
    ($name:ident) => {
        impl Clone for $name {
            fn clone(&self) -> Self {
                Self {
                    inner: self.inner.clone(),
                }
            }
        }

        impl $name {
            /// Escape hatch: the underlying Qt `QWidget` pointer.
            pub fn qobject_ptr(&self) -> *mut Void {
                self.inner.raw() as *mut Void
            }
        }

        impl IntoWidget for $name {
            fn component(&self) -> &Rc<Component> {
                &self.inner
            }
        }
    };
}

/// A horizontal row of widgets.
pub struct Row {
    inner: Rc<Component>,
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
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TableView {
            inner,
            table: model.state.clone(),
            selection,
        }
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
    inner: Rc<Component>,
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
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TableView {
            inner,
            table: model.state.clone(),
            selection,
        }
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
}

widget_wrapper!(Column);

/// A grid layout.
pub struct Grid {
    inner: Rc<Component>,
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

/// A label.
pub struct Label {
    inner: Rc<Component>,
}

impl Label {
    /// Set the label text.
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::label_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// Read the current label text.
    pub fn text(&self) -> String {
        unsafe { ffi::label_text(self.inner.raw()) }
    }

    /// Bind the label text to a signal. The binding lives in the component's
    /// owner and is released when the widget is destroyed.
    pub fn bind_text(&self, signal: &Signal<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_label_text(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(Label);

/// A push button.
pub struct Button {
    inner: Rc<Component>,
}

impl Button {
    /// A stream of click events.
    pub fn clicked(&self) -> EventStream<()> {
        if self.inner.clicked_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_clicked_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.clicked_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.clicked_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a click handler. The subscription lives in the button's
    /// `Owner` and is released with the widget tree.
    pub fn on_click<F>(&self, f: F) -> &Self
    where
        F: FnMut(&()) + 'static,
    {
        self.inner.owner.borrow_mut().add(self.clicked().observe(f));
        self
    }

    /// Enable or disable the button.
    pub fn set_enabled(&self, enabled: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_enabled(self.inner.raw(), enabled) };
        }
    }

    /// Bind the enabled state to a signal.
    pub fn bind_enabled(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |enabled| {
            if let Some(this) = weak.upgrade() {
                this.set_button_enabled(*enabled);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Simulate a click (used by tests and headless smoke runs).
    pub fn click(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::button_click(self.inner.raw()) };
        }
    }
}

widget_wrapper!(Button);

/// A single-line text input.
pub struct LineEdit {
    inner: Rc<Component>,
}

impl LineEdit {
    /// The current text.
    pub fn text(&self) -> String {
        unsafe { ffi::line_edit_text(self.inner.raw()) }
    }

    /// Set the text programmatically (user edits are observable via
    /// [`LineEdit::text_changed`]).
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::line_edit_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// A stream of user text changes.
    pub fn text_changed(&self) -> EventStream<String> {
        if self.inner.text_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_text_changed_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.text_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.text_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a text-change handler (user edits; programmatic writes made
    /// through controlled bindings are suppressed).
    pub fn on_text_change<F>(&self, f: F) -> &Self
    where
        F: FnMut(&String) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.text_changed().observe(f));
        self
    }

    /// Controlled binding: the widget text follows the signal.
    pub fn bind_text(&self, signal: &Signal<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_text_guarded(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Two-way binding between the widget text and a `Var`, with a guard that
    /// prevents feedback loops.
    pub fn bind_text_two_way(&self, var: &Var<String>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = var.signal().observe(move |text| {
            if let Some(this) = weak.upgrade() {
                this.set_text_guarded(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);

        let var_rc = var.clone();
        let subscription = self.text_changed().observe(move |text| {
            if var_rc.value().as_ref() != text {
                var_rc.set(text.clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(LineEdit);

/// A checkbox.
pub struct CheckBox {
    inner: Rc<Component>,
}

impl CheckBox {
    /// Whether the box is checked.
    pub fn checked(&self) -> bool {
        unsafe { ffi::checkbox_checked(self.inner.raw()) }
    }

    /// Set the checked state programmatically.
    pub fn set_checked(&self, checked: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::checkbox_set_checked(self.inner.raw(), checked) };
        }
    }

    /// A stream of toggle events carrying the new checked state.
    pub fn toggled(&self) -> EventStream<bool> {
        if self.inner.toggled_sink.borrow().is_none() {
            unsafe {
                ffi::widget_set_toggled_cb(
                    self.inner.raw(),
                    &*self.inner as *const Component as *mut Void,
                );
            }
            *self.inner.toggled_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.toggled_sink.borrow().as_ref().unwrap().stream()
    }

    /// Register a toggle handler carrying the new checked state.
    pub fn on_toggle<F>(&self, f: F) -> &Self
    where
        F: FnMut(&bool) + 'static,
    {
        self.inner.owner.borrow_mut().add(self.toggled().observe(f));
        self
    }

    /// Bind the checked state to a signal.
    pub fn bind_checked(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |checked| {
            if let Some(this) = weak.upgrade() {
                this.set_checkbox_checked(*checked);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(CheckBox);

/// A QAction: a command that can be triggered from a menu, a toolbar, or a
/// shortcut.
pub struct Action {
    inner: Rc<ActionState>,
}

impl Clone for Action {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Action {
    /// A stream of trigger events.
    pub fn triggered(&self) -> EventStream<()> {
        if self.inner.triggered_sink.borrow().is_none() {
            unsafe {
                ffi::action_set_triggered_cb(
                    self.inner.raw(),
                    &*self.inner as *const ActionState as *mut Void,
                );
            }
            *self.inner.triggered_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner
            .triggered_sink
            .borrow()
            .as_ref()
            .unwrap()
            .stream()
    }

    /// Register a trigger handler.
    pub fn on_trigger<F>(&self, f: F) -> &Self
    where
        F: FnMut(&()) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.triggered().observe(f));
        self
    }

    /// Enable or disable the action.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }

    /// Bind the enabled state to a signal.
    pub fn bind_enabled(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |enabled| {
            if let Some(this) = weak.upgrade() {
                this.set_enabled(*enabled);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Set the action text.
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// Set a keyboard shortcut, e.g. `"Ctrl+Q"`.
    pub fn set_shortcut(&self, shortcut: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_shortcut(self.inner.raw(), &shortcut.into()) };
        }
    }

    /// Set a keyboard shortcut and return the action, for chaining.
    pub fn shortcut(self, shortcut: impl Into<String>) -> Self {
        self.set_shortcut(shortcut);
        self
    }

    /// Trigger the action programmatically (used by tests and smoke runs).
    pub fn trigger(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::action_trigger(self.inner.raw()) };
        }
    }
}

/// A menu bar attached to a window.
pub struct MenuBar {
    inner: Rc<Component>,
}

impl MenuBar {
    /// Create a menu in this bar.
    pub fn menu(&self, title: impl Into<String>) -> Menu {
        Menu {
            inner: unsafe {
                Component::from_raw_child(
                    ffi::menu_new(&title.into(), self.inner.raw()),
                    &self.inner,
                )
            },
        }
    }

    /// Create a menu and hand it to `f`; `f`'s value (typically action
    /// handles) is passed through so menus build inline.
    pub fn menu_with<R, F: FnOnce(&Menu) -> R>(&self, title: impl Into<String>, f: F) -> R {
        let menu = self.menu(title);
        f(&menu)
    }
}

widget_wrapper!(MenuBar);

/// A menu of actions.
pub struct Menu {
    inner: Rc<Component>,
}

impl Menu {
    /// Create an action in this menu.
    pub fn action(&self, text: impl Into<String>) -> Action {
        let inner =
            unsafe { ActionState::from_raw(ffi::action_new(&text.into(), self.inner.raw())) };
        self.inner.retain(inner.clone());
        Action { inner }
    }

    /// Add a separator.
    pub fn separator(&self) -> &Self {
        if self.inner.is_alive() {
            unsafe { ffi::menu_add_separator(self.inner.raw()) };
        }
        self
    }
}

widget_wrapper!(Menu);

/// A toolbar attached to a window.
pub struct ToolBar {
    inner: Rc<Component>,
}

impl ToolBar {
    /// Add an action to this toolbar.
    pub fn add(&self, action: &Action) {
        if self.inner.is_alive() {
            unsafe { ffi::toolbar_add_action(self.inner.raw(), action.inner.raw()) };
        }
    }
}

widget_wrapper!(ToolBar);

/// A `QListView` bound to a [`StringListModel`].
pub struct ListView {
    inner: Rc<Component>,
    model: Rc<ModelState>,
    selection: Rc<SelectionBridge>,
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
    pub fn qobject_ptr(&self) -> *mut Void {
        self.inner.raw() as *mut Void
    }
}

/// Internal bridge for forwarding `QItemSelectionModel` changes to a stream.
pub(crate) struct SelectionBridge {
    view: *mut ffi::Widget,
    sink: RefCell<Option<Rc<Sink<Vec<usize>>>>>,
}

impl IntoWidget for ListView {
    fn component(&self) -> &Rc<Component> {
        &self.inner
    }
}

struct ModelState {
    model: *mut ffi::Model,
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
    state: Rc<ModelState>,
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

struct TableState {
    table: *mut ffi::TableModel,
    list: Rc<ListModel<Vec<String>>>,
    subscription: RefCell<Option<Subscription>>,
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
    state: Rc<TableState>,
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
            }),
        }
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
    inner: Rc<Component>,
    table: Rc<TableState>,
    selection: Rc<SelectionBridge>,
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

/// Buttons shown in a [`MessageBox`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoxButtons {
    Ok,
    OkCancel,
    YesNo,
}

/// The button a [`MessageBox`] was closed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoxResult {
    Ok,
    Cancel,
    Yes,
    No,
}

const QMESSAGEBOX_OK: i32 = 0x0000_0400;
const QMESSAGEBOX_CANCEL: i32 = 0x0040_0000;
const QMESSAGEBOX_YES: i32 = 0x0000_4000;
const QMESSAGEBOX_NO: i32 = 0x0000_8000;
const QDIALOG_REJECTED: i32 = 0;
const QDIALOG_ACCEPTED: i32 = 1;

fn map_dialog_result(value: i32) -> MessageBoxResult {
    match value {
        QMESSAGEBOX_OK | QDIALOG_ACCEPTED => MessageBoxResult::Ok,
        QMESSAGEBOX_CANCEL | QDIALOG_REJECTED => MessageBoxResult::Cancel,
        QMESSAGEBOX_YES => MessageBoxResult::Yes,
        QMESSAGEBOX_NO => MessageBoxResult::No,
        _ => MessageBoxResult::Ok,
    }
}

struct DialogState {
    ptr: *mut ffi::Dialog,
    result_sink: RefCell<Option<Rc<Sink<MessageBoxResult>>>>,
    finished: Cell<bool>,
    parent: std::rc::Weak<Component>,
    self_weak: std::rc::Weak<DialogState>,
    retained_id: Cell<Option<RetainedId>>,
}

impl Drop for DialogState {
    fn drop(&mut self) {
        unsafe { ffi::dialog_drop(self.ptr) };
    }
}

impl DialogState {
    fn on_finished(&self) {
        if self.finished.get() {
            return;
        }
        let keep_alive = self.self_weak.upgrade();
        self.finished.set(true);
        let result = map_dialog_result(unsafe { ffi::dialog_last_result(self.ptr) });
        if let Some(sink) = self.result_sink.borrow().as_ref() {
            sink.send(result);
        }
        if let (Some(parent), Some(id)) = (self.parent.upgrade(), self.retained_id.take()) {
            parent.release(id);
        }
        drop(keep_alive);
    }
}

/// A non-modal message box whose result arrives on a stream.
pub struct MessageBox {
    inner: Rc<DialogState>,
}

impl Clone for MessageBox {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl MessageBox {
    /// Create a message box parented to `window`.
    pub fn new(
        window: &Window,
        title: impl Into<String>,
        text: impl Into<String>,
        buttons: MessageBoxButtons,
    ) -> Self {
        let buttons = match buttons {
            MessageBoxButtons::Ok => 0,
            MessageBoxButtons::OkCancel => 1,
            MessageBoxButtons::YesNo => 2,
        };
        let ptr = unsafe {
            ffi::dialog_message_new(window.inner.raw(), &title.into(), &text.into(), buttons)
        };
        let state = Rc::new_cyclic(|self_weak| DialogState {
            ptr,
            result_sink: RefCell::new(None),
            finished: Cell::new(false),
            parent: Rc::downgrade(&window.inner),
            self_weak: self_weak.clone(),
            retained_id: Cell::new(None),
        });
        state
            .retained_id
            .set(Some(window.inner.retain(state.clone())));
        unsafe {
            ffi::dialog_set_finished_cb(ptr, &*state as *const DialogState as *mut Void);
        }
        Self { inner: state }
    }

    /// Show the box (non-modal).
    pub fn show(&self) {
        unsafe { ffi::dialog_show(self.inner.ptr) };
    }

    /// Programmatically click the default button (used by tests).
    pub fn accept(&self) {
        unsafe { ffi::dialog_accept(self.inner.ptr) };
    }

    /// Close the box without choosing a button.
    pub fn close(&self) {
        unsafe { ffi::dialog_close(self.inner.ptr) };
    }

    /// A stream that emits the closing result exactly once.
    pub fn result(&self) -> EventStream<MessageBoxResult> {
        if self.inner.result_sink.borrow().is_none() {
            *self.inner.result_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.result_sink.borrow().as_ref().unwrap().stream()
    }
}

struct FileDialogState {
    ptr: *mut ffi::FileDialog,
    result_sink: RefCell<Option<Rc<Sink<Option<String>>>>>,
    finished: Cell<bool>,
    parent: std::rc::Weak<Component>,
    self_weak: std::rc::Weak<FileDialogState>,
    retained_id: Cell<Option<RetainedId>>,
}

impl Drop for FileDialogState {
    fn drop(&mut self) {
        unsafe { ffi::filedialog_drop(self.ptr) };
    }
}

impl FileDialogState {
    fn on_finished(&self) {
        if self.finished.get() {
            return;
        }
        let keep_alive = self.self_weak.upgrade();
        self.finished.set(true);
        let path = unsafe { ffi::filedialog_selected_file(self.ptr) };
        let selected = if path.is_empty() { None } else { Some(path) };
        if let Some(sink) = self.result_sink.borrow().as_ref() {
            sink.send(selected);
        }
        if let (Some(parent), Some(id)) = (self.parent.upgrade(), self.retained_id.take()) {
            parent.release(id);
        }
        drop(keep_alive);
    }
}

/// A non-modal "open file" dialog whose selection arrives on a stream.
pub struct FileDialog {
    inner: Rc<FileDialogState>,
}

impl Clone for FileDialog {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl FileDialog {
    /// Create an open-file dialog parented to `window`.
    pub fn open(window: &Window, title: impl Into<String>) -> Self {
        let ptr = unsafe { ffi::filedialog_open_new(window.inner.raw(), &title.into()) };
        let state = Rc::new_cyclic(|self_weak| FileDialogState {
            ptr,
            result_sink: RefCell::new(None),
            finished: Cell::new(false),
            parent: Rc::downgrade(&window.inner),
            self_weak: self_weak.clone(),
            retained_id: Cell::new(None),
        });
        state
            .retained_id
            .set(Some(window.inner.retain(state.clone())));
        unsafe {
            ffi::filedialog_set_finished_cb(ptr, &*state as *const FileDialogState as *mut Void);
        }
        Self { inner: state }
    }

    /// Show the dialog (non-modal).
    pub fn show(&self) {
        unsafe { ffi::filedialog_show(self.inner.ptr) };
    }

    /// Programmatically accept (native dialogs may require an event loop).
    pub fn accept(&self) {
        unsafe { ffi::filedialog_accept(self.inner.ptr) };
    }

    /// Programmatically close without selecting (headless-friendly).
    pub fn close(&self) {
        unsafe { ffi::filedialog_close(self.inner.ptr) };
    }

    /// A stream that emits the selected path (or `None` if nothing was
    /// chosen) exactly once.
    pub fn result(&self) -> EventStream<Option<String>> {
        if self.inner.result_sink.borrow().is_none() {
            *self.inner.result_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.result_sink.borrow().as_ref().unwrap().stream()
    }
}

/// A persistent key/value store backed by Qt's `QSettings`.
///
/// Values are strings; `set` writes through to the platform configuration
/// store and [`Settings::sync`] flushes it to disk.
pub struct Settings {
    inner: Rc<SettingsState>,
}

struct SettingsState {
    ptr: *mut ffi::Settings,
}

impl Clone for Settings {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Settings {
    /// Open the application configuration store for `organization` and
    /// `application`.
    pub fn new(organization: impl Into<String>, application: impl Into<String>) -> Self {
        Self {
            inner: Rc::new(SettingsState {
                ptr: unsafe { ffi::settings_new(&organization.into(), &application.into()) },
            }),
        }
    }

    /// The value for `key`, if set.
    pub fn value(&self, key: &str) -> Option<String> {
        if !self.contains(key) {
            return None;
        }
        Some(unsafe { ffi::settings_value(self.inner.ptr, key) })
    }

    /// Set `key` to `value`.
    pub fn set(&self, key: &str, value: &str) {
        unsafe { ffi::settings_set(self.inner.ptr, key, value) };
    }

    /// Remove `key`.
    pub fn remove(&self, key: &str) {
        unsafe { ffi::settings_remove(self.inner.ptr, key) };
    }

    /// Whether `key` is present.
    pub fn contains(&self, key: &str) -> bool {
        unsafe { ffi::settings_contains(self.inner.ptr, key) }
    }

    /// Flush pending writes to disk.
    pub fn sync(&self) {
        unsafe { ffi::settings_sync(self.inner.ptr) };
    }
}

impl Drop for SettingsState {
    fn drop(&mut self) {
        unsafe { ffi::settings_drop(self.ptr) };
    }
}
