//! Laminar-style retained widget composition.
//!
//! Widget constructors return inert [`View`] descriptions. A view is mounted
//! once with [`Window::mount`]; reactive modifiers install subscriptions owned
//! by the resulting widget tree.
//!
//! ```no_run
//! use yse_model::Var;
//! use yse_ui::*;
//!
//! let app = Application::init();
//! let window = Window::new();
//! let count = Var::new(0u32);
//! let caption = count.signal().map(|count| format!("Count: {count}"));
//! window.mount(column((
//!     label(caption),
//!     row((
//!         spacer(),
//!         button("Increment")
//!             .on_click(clone!(count => move |_| count.set(*count.value() + 1))),
//!     )),
//! )));
//! ```

use crate::bridge as ffi;
use crate::component::Component;
use crate::{
    Button, CheckBox, ComboBox, DateEdit, DateTimeEdit, DiskMap, Label, LineChart, LineEdit,
    ListView, Menu, ProgressBar, SelectionBridge, Slider, SpinBox, StackedWidget, StringListModel,
    StringTableModel, TabWidget, TableView, TimeEdit, TreeModel, TreeView, Window,
};
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Subscription;
use yse_model::{Signal, Var};

/// A retained widget description that can be mounted into a window.
///
/// This is Yse's equivalent of a Laminar `ReactiveElement`: constructors
/// return inert values, modifiers configure those values, and [`Window::mount`]
/// performs the Qt side effects in one place.
pub trait View {
    /// Handles returned after this view is mounted.
    type Output;

    /// Mount this view below `context`.
    #[doc(hidden)]
    fn mount_in(self, context: &MountContext) -> Self::Output;
}

/// Parent information supplied while a [`View`] is mounted.
#[doc(hidden)]
pub struct MountContext {
    parent: Rc<Component>,
    is_layout: bool,
}

impl MountContext {
    fn child(&self, ptr: *mut ffi::Widget) -> Rc<Component> {
        let child = unsafe { Component::from_raw_child(ptr, &self.parent) };
        if self.is_layout {
            unsafe { ffi::layout_add(self.parent.raw(), child.raw()) };
        }
        child
    }
}

/// A static or reactive string property.
#[doc(hidden)]
pub enum TextValue {
    Static(String),
    Reactive(Signal<String>),
}

/// Convert a static string or a `Signal<String>` into a reactive property.
pub trait IntoTextValue {
    /// Perform the conversion.
    fn into_text_value(self) -> TextValue;
}

impl IntoTextValue for String {
    fn into_text_value(self) -> TextValue {
        TextValue::Static(self)
    }
}

impl IntoTextValue for &str {
    fn into_text_value(self) -> TextValue {
        TextValue::Static(self.to_owned())
    }
}

impl IntoTextValue for Signal<String> {
    fn into_text_value(self) -> TextValue {
        TextValue::Reactive(self)
    }
}

/// A static or reactive boolean property.
#[doc(hidden)]
pub enum BoolValue {
    Static(bool),
    Reactive(Signal<bool>),
}

/// Convert a boolean or `Signal<bool>` into a reactive property.
pub trait IntoBoolValue {
    /// Perform the conversion.
    fn into_bool_value(self) -> BoolValue;
}

impl IntoBoolValue for bool {
    fn into_bool_value(self) -> BoolValue {
        BoolValue::Static(self)
    }
}

impl IntoBoolValue for Signal<bool> {
    fn into_bool_value(self) -> BoolValue {
        BoolValue::Reactive(self)
    }
}

/// A static or reactive integer property.
#[doc(hidden)]
pub enum IntValue {
    Static(i32),
    Reactive(Signal<i32>),
    Controlled(Var<i32>),
}

/// Convert an `i32`, `Signal<i32>`, or `Var<i32>` into a reactive property.
pub trait IntoIntValue {
    /// Perform the conversion.
    fn into_int_value(self) -> IntValue;
}

impl IntoIntValue for i32 {
    fn into_int_value(self) -> IntValue {
        IntValue::Static(self)
    }
}

impl IntoIntValue for Signal<i32> {
    fn into_int_value(self) -> IntValue {
        IntValue::Reactive(self)
    }
}

impl IntoIntValue for Var<i32> {
    fn into_int_value(self) -> IntValue {
        IntValue::Controlled(self)
    }
}

enum LayoutAxis {
    Row,
    Column,
}

/// A row or column whose children are mounted in tuple order.
pub struct LayoutView<C> {
    axis: LayoutAxis,
    children: C,
}

/// Describe a horizontal layout.
pub fn row<C: View>(children: C) -> LayoutView<C> {
    LayoutView {
        axis: LayoutAxis::Row,
        children,
    }
}

/// Describe a vertical layout.
pub fn column<C: View>(children: C) -> LayoutView<C> {
    LayoutView {
        axis: LayoutAxis::Column,
        children,
    }
}

impl<C: View> View for LayoutView<C> {
    type Output = C::Output;

    fn mount_in(self, context: &MountContext) -> Self::Output {
        let ptr = match self.axis {
            LayoutAxis::Row => unsafe { ffi::widget_new_row(context.parent.raw()) },
            LayoutAxis::Column => unsafe { ffi::widget_new_column(context.parent.raw()) },
        };
        let layout = context.child(ptr);
        self.children.mount_in(&MountContext {
            parent: layout,
            is_layout: true,
        })
    }
}

mod views;
pub use views::{
    ButtonView, CheckBoxView, ComboBoxView, LabelView, LineEditView, ProgressBarView, SliderView,
    SpinBoxView, button, checkbox, combo_box, label, line_edit, progress_bar, slider, spin_box,
};
pub struct SpacerView;

/// Describe a stretch spacer.
pub fn spacer() -> SpacerView {
    SpacerView
}

impl View for SpacerView {
    type Output = ();

    fn mount_in(self, context: &MountContext) {
        assert!(context.is_layout, "spacers must live inside a layout");
        unsafe { ffi::layout_add_spacer(context.parent.raw()) };
    }
}

impl View for () {
    type Output = ();

    fn mount_in(self, _context: &MountContext) {}
}

macro_rules! tuple_views {
    ($(($($view:ident),+)),+ $(,)?) => {$(
        impl<$($view: View),+> View for ($($view,)+) {
            type Output = ($($view::Output,)+);

            #[allow(non_snake_case)]
            fn mount_in(self, context: &MountContext) -> Self::Output {
                let ($($view,)+) = self;
                ($($view.mount_in(context),)+)
            }
        }
    )+};
}

tuple_views! {
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParentKind {
    Window,
    Layout,
}

/// A scoped builder for a retained Qt widget tree.
///
/// A `Ui` value can only create widgets below its own parent. Use a nested
/// `row` or `column` before creating leaf widgets; this mirrors Qt's layout
/// requirement in the public API rather than relying on ambient state.
pub struct Ui {
    kind: ParentKind,
    parent: Rc<Component>,
}

impl Ui {
    fn layout_parent(&self) -> &Rc<Component> {
        assert_eq!(
            self.kind,
            ParentKind::Layout,
            "leaf widgets must live inside a row() or column(); the top of a window tree should be a layout"
        );
        &self.parent
    }

    fn leaf(&self, ptr: *mut ffi::Widget) -> Rc<Component> {
        let parent = self.layout_parent();
        let inner = unsafe { Component::from_raw_child(ptr, parent) };
        unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
        inner
    }

    fn layout<R, F: FnOnce(&Ui) -> R>(
        &self,
        new_container: unsafe fn(*mut ffi::Widget) -> *mut ffi::Widget,
        f: F,
    ) -> R {
        let inner =
            unsafe { Component::from_raw_child(new_container(self.parent.raw()), &self.parent) };
        if self.kind == ParentKind::Layout {
            unsafe { ffi::layout_add(self.parent.raw(), inner.raw()) };
        }
        f(&Ui {
            kind: ParentKind::Layout,
            parent: inner,
        })
    }

    /// Nest children into a horizontal row.
    pub fn row<R, F: FnOnce(&Ui) -> R>(&self, f: F) -> R {
        self.layout(ffi::widget_new_row, f)
    }

    /// Nest children into a vertical column.
    pub fn column<R, F: FnOnce(&Ui) -> R>(&self, f: F) -> R {
        self.layout(ffi::widget_new_column, f)
    }

    /// Create a label in this layout.
    pub fn label(&self, text: impl Into<String>) -> Label {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_label(&text.into(), parent.raw()) });
        Label { inner }
    }

    /// Create a button in this layout.
    pub fn button(&self, text: impl Into<String>) -> Button {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_button(&text.into(), parent.raw()) });
        Button { inner }
    }

    /// Create a line edit in this layout.
    pub fn line_edit(&self, text: impl Into<String>) -> LineEdit {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_line_edit(&text.into(), parent.raw()) });
        LineEdit { inner }
    }

    /// Create a native Qt date/time editor with a calendar popup.
    pub fn date_time_edit(&self, iso_datetime: impl Into<String>) -> DateTimeEdit {
        let parent = self.layout_parent();
        let inner =
            self.leaf(unsafe { ffi::widget_new_datetime_edit(&iso_datetime.into(), parent.raw()) });
        DateTimeEdit { inner }
    }

    /// Create a native Qt calendar date editor.
    pub fn date_edit(&self, iso_date: impl Into<String>) -> DateEdit {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_date_edit(&iso_date.into(), parent.raw()) });
        DateEdit { inner }
    }

    /// Create a native Qt time editor with hour and minute spin controls.
    pub fn time_edit(&self, iso_time: impl Into<String>) -> TimeEdit {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_time_edit(&iso_time.into(), parent.raw()) });
        TimeEdit { inner }
    }

    /// Create a proportional disk-usage overview.
    pub fn disk_map(&self) -> DiskMap {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_disk_map(parent.raw()) });
        DiskMap { inner }
    }

    /// Create a checkbox in this layout.
    pub fn checkbox(&self, text: impl Into<String>) -> CheckBox {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_checkbox(&text.into(), parent.raw()) });
        CheckBox { inner }
    }

    /// Create a combo box with `items` in this layout.
    pub fn combo_box<I, S>(&self, items: I) -> ComboBox
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe {
            ffi::widget_new_combo(items.into_iter().map(Into::into).collect(), parent.raw())
        });
        ComboBox { inner }
    }

    /// Create a numeric spinner (default range `0..=100`) in this layout.
    pub fn spin_box(&self, value: i32) -> SpinBox {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_spin_box(0, 100, value, parent.raw()) });
        SpinBox { inner }
    }

    /// Create a horizontal slider (default range `0..=100`) in this layout.
    pub fn slider(&self, value: i32) -> Slider {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_slider(0, 100, value, parent.raw()) });
        Slider { inner }
    }

    /// Create a progress bar (default range `0..=100`) in this layout.
    pub fn progress_bar(&self, value: i32) -> ProgressBar {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_progress_bar(0, 100, value, parent.raw()) });
        ProgressBar { inner }
    }

    /// Create a tabbed container in this layout.
    pub fn tab_widget(&self) -> TabWidget {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_tab_widget(parent.raw()) });
        TabWidget { inner }
    }

    /// Create a stacked content widget in this layout.
    pub fn stacked_widget(&self) -> StackedWidget {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_stacked_widget(parent.raw()) });
        StackedWidget { inner }
    }

    /// Create a painted line chart in this layout.
    pub fn line_chart(&self) -> LineChart {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_line_chart(parent.raw()) });
        LineChart { inner }
    }

    /// Add a stretch spacer to this layout.
    pub fn spacer(&self) {
        let parent = self.layout_parent();
        unsafe { ffi::layout_add_spacer(parent.raw()) };
    }

    /// Create a list view bound to `model` in this layout.
    pub fn list_view(&self, model: &StringListModel) -> ListView {
        let parent = self.layout_parent();
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_list_view(model.state.model, parent.raw()),
                parent,
            )
        };
        unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
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

    /// Create a table view bound to `model` in this layout.
    pub fn table_view(&self, model: &StringTableModel) -> TableView {
        let parent = self.layout_parent();
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_table_view(model.state.table, parent.raw()),
                parent,
            )
        };
        unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
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
        let parent = self.layout_parent();
        let inner = unsafe {
            Component::from_raw_child(
                ffi::widget_new_tree_view(model.state.tree, parent.raw()),
                parent,
            )
        };
        unsafe { ffi::layout_add(parent.raw(), inner.raw()) };
        let selection = Rc::new(SelectionBridge {
            view: inner.raw(),
            sink: RefCell::new(None),
            created_on: std::thread::current().id(),
        });
        inner.retain(model.state.clone());
        inner.retain(selection.clone());
        TreeView { inner, selection }
    }

    /// Retain a non-widget subscription until this subtree is destroyed.
    pub fn own(&self, subscription: Subscription) {
        self.parent.owner.borrow_mut().add(subscription);
    }
}

impl Window {
    /// Mount a Laminar-style retained view description into this window.
    pub fn mount<V: View>(&self, view: V) -> V::Output {
        view.mount_in(&MountContext {
            parent: self.inner.clone(),
            is_layout: false,
        })
    }

    /// Return the root builder for this window's widget tree.
    ///
    /// Start with a [`Ui::row`] or [`Ui::column`]; their closure return values
    /// let callers retain only the handles needed for imperative operations or
    /// tests.
    pub fn ui(&self) -> Ui {
        Ui {
            kind: ParentKind::Window,
            parent: self.inner.clone(),
        }
    }

    /// Create a standalone menu (not attached to a menu bar) that can be
    /// shown as a popup, e.g. for context menus.
    pub fn menu(&self, title: impl Into<String>) -> Menu {
        Menu {
            inner: unsafe {
                Component::from_raw_child(
                    ffi::window_menu_new(&title.into(), self.inner.raw()),
                    &self.inner,
                )
            },
        }
    }
}

impl TabWidget {
    /// Append a tab and return a builder scoped to its page. Widgets created
    /// through the returned [`Ui`] are owned by the page and released with the
    /// tab widget.
    pub fn add_tab(&self, label: impl Into<String>) -> Ui {
        let page = unsafe {
            Component::from_raw_child(
                ffi::tab_widget_add_page(self.inner.raw(), &label.into()),
                &self.inner,
            )
        };
        Ui {
            kind: ParentKind::Layout,
            parent: page,
        }
    }

    /// Select the tab at `index` (used by tests and headless smoke runs).
    pub fn set_current(&self, index: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::tab_widget_set_current(self.inner.raw(), index as i32) };
        }
    }

    /// The number of tabs.
    pub fn count(&self) -> usize {
        unsafe { ffi::tab_widget_count(self.inner.raw()) as usize }
    }
}

impl StackedWidget {
    /// Append a page and return a builder scoped to it.
    pub fn add_page(&self) -> Ui {
        let page = unsafe {
            Component::from_raw_child(ffi::stacked_add_page(self.inner.raw()), &self.inner)
        };
        Ui {
            kind: ParentKind::Layout,
            parent: page,
        }
    }

    /// Show the page at `index`.
    pub fn set_current(&self, index: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::stacked_set_current(self.inner.raw(), index as i32) };
        }
    }

    /// The number of pages.
    pub fn count(&self) -> usize {
        unsafe { ffi::stacked_count(self.inner.raw()) as usize }
    }
}
