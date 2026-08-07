//! Tabbed container wrapper (pages are built with [`crate::Ui`]).

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

pub struct TabWidget {
    pub(crate) inner: Rc<Component>,
}

widget_wrapper!(TabWidget);
