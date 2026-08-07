//! `yse-ui` binds [`yse_model`] to a retained Qt Widgets tree.
//!
//! Components are plain Rust wrappers around a small hand-written C++ shim.
//! Every component owns its subscriptions through an `Owner` that is tied to
//! the Qt object lifetime: when the widget is destroyed, all of its bindings
//! are released automatically.

mod action;
mod app;
mod bridge;
mod callback;
mod component;
mod dsl;
mod qt_object;
mod widgets;

pub(crate) use widgets::SelectionBridge;

pub use app::{Application, QtGuiScheduler, QtTimer};
pub use dsl::{
    BoolValue, ButtonView, CheckBoxView, ComboBoxView, IntoBoolValue, IntoIntValue, IntoTextValue,
    LabelView, LayoutView, LineEditView, MountContext, ProgressBarView, SliderView, SpacerView,
    SpinBoxView, TextValue, Ui, View, button, checkbox, column, combo_box, label, line_edit,
    progress_bar, row, slider, spacer, spin_box,
};
pub use widgets::*;

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
