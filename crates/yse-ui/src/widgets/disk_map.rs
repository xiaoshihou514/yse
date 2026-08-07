//! Painted disk-usage overview wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

pub struct DiskMap {
    pub(crate) inner: Rc<Component>,
}

impl DiskMap {
    /// Replace the displayed segments. Shares are percentages and need not add
    /// up exactly to 100; the widget normalizes them for display.
    pub fn set_segments(
        &self,
        labels: impl IntoIterator<Item = String>,
        shares: impl IntoIterator<Item = f64>,
    ) {
        if self.inner.is_alive() {
            unsafe {
                ffi::disk_map_set_segments(
                    self.inner.raw(),
                    labels.into_iter().collect(),
                    shares.into_iter().collect(),
                )
            };
        }
    }
}

widget_wrapper!(DiskMap);
