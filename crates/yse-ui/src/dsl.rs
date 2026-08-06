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
    Button, CheckBox, Label, LineEdit, ListView, SelectionBridge, StringListModel,
    StringTableModel, TableView, Window, ffi,
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

    /// Create a checkbox in this layout.
    pub fn checkbox(&self, text: impl Into<String>) -> CheckBox {
        let parent = self.layout_parent();
        let inner = self.leaf(unsafe { ffi::widget_new_checkbox(&text.into(), parent.raw()) });
        CheckBox { inner }
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
