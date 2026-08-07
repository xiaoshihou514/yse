//! Stacked content widget for rail-style navigation.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

/// A stack of pages where exactly one is visible at a time; pages are built
/// with [`crate::Ui`].
pub struct StackedWidget {
    pub(crate) inner: Rc<Component>,
}

widget_wrapper!(StackedWidget);
