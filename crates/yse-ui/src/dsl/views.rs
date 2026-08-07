//! Declarative view descriptions for the Laminar-style DSL.

use super::{
    BoolValue, IntValue, IntoBoolValue, IntoIntValue, IntoTextValue, MountContext, TextValue, View,
};
use crate::bridge as ffi;
use crate::{Button, CheckBox, ComboBox, Label, LineEdit, ProgressBar, Slider, SpinBox};
use yse_model::{Signal, Var};

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
