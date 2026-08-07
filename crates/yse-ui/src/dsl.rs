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

use crate::component::Component;
use crate::{
    Button, CheckBox, ComboBox, DateEdit, DateTimeEdit, DiskMap, Label, LineChart, LineEdit,
    ListView, ProgressBar, SelectionBridge, Slider, SpinBox, StringListModel, StringTableModel,
    TabWidget, TableView, TimeEdit, Window, ffi,
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

/// A label description.
pub struct LabelView {
    text: TextValue,
}

/// Describe a label. A `Signal<String>` binds reactively.
pub fn label(text: impl IntoTextValue) -> LabelView {
    LabelView {
        text: text.into_text_value(),
    }
}

impl View for LabelView {
    type Output = Label;

    fn mount_in(self, context: &MountContext) -> Label {
        let initial = match &self.text {
            TextValue::Static(text) => text.clone(),
            TextValue::Reactive(signal) => signal.value().as_ref().clone(),
        };
        let inner = context.child(unsafe { ffi::widget_new_label(&initial, context.parent.raw()) });
        let label = Label { inner };
        if let TextValue::Reactive(signal) = self.text {
            label.bind_text(&signal);
        }
        label
    }
}

type ClickObserver = Box<dyn FnMut(&())>;
type ValueObserver = Box<dyn FnMut(&i32)>;

/// A push-button description.
pub struct ButtonView {
    text: String,
    enabled: Option<BoolValue>,
    clicks: Vec<ClickObserver>,
}

/// Describe a push button.
pub fn button(text: impl Into<String>) -> ButtonView {
    ButtonView {
        text: text.into(),
        enabled: None,
        clicks: Vec::new(),
    }
}

impl ButtonView {
    /// Set or reactively bind the enabled property.
    pub fn enabled(mut self, enabled: impl IntoBoolValue) -> Self {
        self.enabled = Some(enabled.into_bool_value());
        self
    }

    /// Attach a click observer owned by the mounted button.
    pub fn on_click(mut self, observer: impl FnMut(&()) + 'static) -> Self {
        self.clicks.push(Box::new(observer));
        self
    }
}

impl View for ButtonView {
    type Output = Button;

    fn mount_in(self, context: &MountContext) -> Button {
        let inner =
            context.child(unsafe { ffi::widget_new_button(&self.text, context.parent.raw()) });
        let button = Button { inner };
        if let Some(enabled) = self.enabled {
            match enabled {
                BoolValue::Static(enabled) => button.set_enabled(enabled),
                BoolValue::Reactive(signal) => button.bind_enabled(&signal),
            }
        }
        for observer in self.clicks {
            button.on_click(observer);
        }
        button
    }
}

enum LineEditValue {
    Static(String),
    Reactive(Signal<String>),
    Controlled(Var<String>),
}

/// A single-line text input description.
pub struct LineEditView {
    value: LineEditValue,
}

/// Describe an uncontrolled text input with an initial value.
pub fn line_edit(text: impl Into<String>) -> LineEditView {
    LineEditView {
        value: LineEditValue::Static(text.into()),
    }
}

impl LineEditView {
    /// Bind the input's value to a read-only signal.
    pub fn value(mut self, signal: Signal<String>) -> Self {
        self.value = LineEditValue::Reactive(signal);
        self
    }

    /// Two-way bind the input's value to a `Var`, like Laminar's controlled
    /// input pattern.
    pub fn controlled(mut self, value: Var<String>) -> Self {
        self.value = LineEditValue::Controlled(value);
        self
    }
}

impl View for LineEditView {
    type Output = LineEdit;

    fn mount_in(self, context: &MountContext) -> LineEdit {
        let initial = match &self.value {
            LineEditValue::Static(text) => text.clone(),
            LineEditValue::Reactive(signal) => signal.value().as_ref().clone(),
            LineEditValue::Controlled(var) => var.value().as_ref().clone(),
        };
        let inner =
            context.child(unsafe { ffi::widget_new_line_edit(&initial, context.parent.raw()) });
        let input = LineEdit { inner };
        match self.value {
            LineEditValue::Static(_) => {}
            LineEditValue::Reactive(signal) => input.bind_text(&signal),
            LineEditValue::Controlled(var) => input.bind_text_two_way(&var),
        }
        input
    }
}

/// A checkbox description.
pub struct CheckBoxView {
    text: String,
    checked: Option<BoolValue>,
}

/// Describe a checkbox.
pub fn checkbox(text: impl Into<String>) -> CheckBoxView {
    CheckBoxView {
        text: text.into(),
        checked: None,
    }
}

impl CheckBoxView {
    /// Set or reactively bind the checked property.
    pub fn checked(mut self, checked: impl IntoBoolValue) -> Self {
        self.checked = Some(checked.into_bool_value());
        self
    }
}

impl View for CheckBoxView {
    type Output = CheckBox;

    fn mount_in(self, context: &MountContext) -> CheckBox {
        let inner =
            context.child(unsafe { ffi::widget_new_checkbox(&self.text, context.parent.raw()) });
        let checkbox = CheckBox { inner };
        if let Some(checked) = self.checked {
            match checked {
                BoolValue::Static(checked) => checkbox.set_checked(checked),
                BoolValue::Reactive(signal) => checkbox.bind_checked(&signal),
            }
        }
        checkbox
    }
}

/// A combo-box description.
pub struct ComboBoxView {
    items: Vec<String>,
    selected: Option<IntValue>,
    changes: Vec<ValueObserver>,
}

/// Describe a combo box with the given item labels.
pub fn combo_box<I, S>(items: I) -> ComboBoxView
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    ComboBoxView {
        items: items.into_iter().map(Into::into).collect(),
        selected: None,
        changes: Vec::new(),
    }
}

impl ComboBoxView {
    /// Set or reactively bind the selected index.
    pub fn value(mut self, value: impl IntoIntValue) -> Self {
        self.selected = Some(value.into_int_value());
        self
    }

    /// Attach a selection-change observer owned by the mounted combo box.
    pub fn on_value_change(mut self, observer: impl FnMut(&i32) + 'static) -> Self {
        self.changes.push(Box::new(observer));
        self
    }
}

impl View for ComboBoxView {
    type Output = ComboBox;

    fn mount_in(self, context: &MountContext) -> ComboBox {
        let inner =
            context.child(unsafe { ffi::widget_new_combo(self.items, context.parent.raw()) });
        let combo = ComboBox { inner };
        if let Some(selected) = self.selected {
            match selected {
                IntValue::Static(index) => combo.set_current_index(index),
                IntValue::Reactive(signal) => combo.bind_value(&signal),
                IntValue::Controlled(var) => combo.bind_value_two_way(&var),
            }
        }
        for observer in self.changes {
            combo.on_value_change(observer);
        }
        combo
    }
}

/// A numeric spinner description.
pub struct SpinBoxView {
    value: IntValue,
    range: (i32, i32),
    changes: Vec<ValueObserver>,
}

/// Describe a numeric spinner (default range `0..=100`).
pub fn spin_box(value: impl IntoIntValue) -> SpinBoxView {
    SpinBoxView {
        value: value.into_int_value(),
        range: (0, 100),
        changes: Vec::new(),
    }
}

impl SpinBoxView {
    /// Constrain the accepted value range.
    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.range = (min, max);
        self
    }

    /// Attach a value-change observer owned by the mounted spinner.
    pub fn on_value_change(mut self, observer: impl FnMut(&i32) + 'static) -> Self {
        self.changes.push(Box::new(observer));
        self
    }
}

impl View for SpinBoxView {
    type Output = SpinBox;

    fn mount_in(self, context: &MountContext) -> SpinBox {
        let initial = match &self.value {
            IntValue::Static(value) => *value,
            IntValue::Reactive(signal) => *signal.value(),
            IntValue::Controlled(var) => *var.value(),
        };
        let inner = context.child(unsafe {
            ffi::widget_new_spin_box(self.range.0, self.range.1, initial, context.parent.raw())
        });
        let spin = SpinBox { inner };
        match self.value {
            IntValue::Static(_) => {}
            IntValue::Reactive(signal) => spin.bind_value(&signal),
            IntValue::Controlled(var) => spin.bind_value_two_way(&var),
        }
        for observer in self.changes {
            spin.on_value_change(observer);
        }
        spin
    }
}

/// A horizontal slider description.
pub struct SliderView {
    value: IntValue,
    range: (i32, i32),
    changes: Vec<ValueObserver>,
}

/// Describe a horizontal slider (default range `0..=100`).
pub fn slider(value: impl IntoIntValue) -> SliderView {
    SliderView {
        value: value.into_int_value(),
        range: (0, 100),
        changes: Vec::new(),
    }
}

impl SliderView {
    /// Constrain the accepted value range.
    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.range = (min, max);
        self
    }

    /// Attach a value-change observer owned by the mounted slider.
    pub fn on_value_change(mut self, observer: impl FnMut(&i32) + 'static) -> Self {
        self.changes.push(Box::new(observer));
        self
    }
}

impl View for SliderView {
    type Output = Slider;

    fn mount_in(self, context: &MountContext) -> Slider {
        let initial = match &self.value {
            IntValue::Static(value) => *value,
            IntValue::Reactive(signal) => *signal.value(),
            IntValue::Controlled(var) => *var.value(),
        };
        let inner = context.child(unsafe {
            ffi::widget_new_slider(self.range.0, self.range.1, initial, context.parent.raw())
        });
        let slider = Slider { inner };
        match self.value {
            IntValue::Static(_) => {}
            IntValue::Reactive(signal) => slider.bind_value(&signal),
            IntValue::Controlled(var) => slider.bind_value_two_way(&var),
        }
        for observer in self.changes {
            slider.on_value_change(observer);
        }
        slider
    }
}

/// A progress-bar description.
pub struct ProgressBarView {
    value: IntValue,
    range: (i32, i32),
}

/// Describe a progress bar (default range `0..=100`).
pub fn progress_bar(value: impl IntoIntValue) -> ProgressBarView {
    ProgressBarView {
        value: value.into_int_value(),
        range: (0, 100),
    }
}

impl ProgressBarView {
    /// Constrain the accepted value range.
    pub fn range(mut self, min: i32, max: i32) -> Self {
        self.range = (min, max);
        self
    }
}

impl View for ProgressBarView {
    type Output = ProgressBar;

    fn mount_in(self, context: &MountContext) -> ProgressBar {
        let initial = match &self.value {
            IntValue::Static(value) => *value,
            IntValue::Reactive(signal) => *signal.value(),
            IntValue::Controlled(var) => *var.value(),
        };
        let inner = context.child(unsafe {
            ffi::widget_new_progress_bar(self.range.0, self.range.1, initial, context.parent.raw())
        });
        let bar = ProgressBar { inner };
        match self.value {
            IntValue::Static(_) => {}
            IntValue::Reactive(signal) => bar.bind_value(&signal),
            IntValue::Controlled(var) => bar.bind_value(&var.signal()),
        }
        bar
    }
}

/// A stretch spacer description.
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
