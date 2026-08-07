//! Painted line chart wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::Signal;

pub struct LineChart {
    pub(crate) inner: Rc<Component>,
}

impl LineChart {
    /// Replace the displayed series with a single line.
    pub fn set_series(&self, points: Vec<f64>) {
        if self.inner.is_alive() {
            unsafe { ffi::line_chart_set_series(self.inner.raw(), points) };
        }
    }

    /// Replace the displayed series with `series` overlaid lines. `points` is
    /// row-major: every `points.len() / series` values form one line.
    pub fn set_series_multi(&self, points: Vec<f64>, series: usize) {
        if self.inner.is_alive() {
            unsafe { ffi::line_chart_set_series_multi(self.inner.raw(), points, series) };
        }
    }

    /// Bind the displayed series to a signal. The binding lives in the
    /// chart's owner and is released with the widget tree.
    pub fn bind_series(&self, signal: &Signal<Vec<f64>>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |points| {
            if let Some(this) = weak.upgrade() {
                this.set_chart_series((*points).clone());
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Bind several overlaid series to a signal of per-line point vectors.
    pub fn bind_series_multi(&self, signal: &Signal<Vec<Vec<f64>>>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |series| {
            if let Some(this) = weak.upgrade() {
                let count = series.len();
                let mut flat = Vec::new();
                for line in series {
                    flat.extend_from_slice(line);
                }
                this.set_chart_series_multi(flat, count);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }
}

widget_wrapper!(LineChart);
