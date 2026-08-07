//! Retained Qt widget wrappers, one module per widget family.

mod window;
pub use window::Window;
mod layout;
pub use layout::{Column, Grid, Row};
mod label;
pub use label::Label;
mod button;
pub use button::Button;
mod line_edit;
pub use line_edit::LineEdit;
mod date_time;
pub use date_time::{DateEdit, DateTimeEdit, TimeEdit};
mod disk_map;
pub use disk_map::DiskMap;
mod checkbox;
pub use checkbox::CheckBox;
mod combo_box;
pub use combo_box::ComboBox;
mod spin_box;
pub use spin_box::SpinBox;
mod slider;
pub use slider::Slider;
mod progress_bar;
pub use progress_bar::ProgressBar;
mod tab_widget;
pub use tab_widget::TabWidget;
mod line_chart;
pub use line_chart::LineChart;
mod menu;
pub use menu::{Action, Menu, MenuBar, ToolBar};
mod list_view;
pub use list_view::{ListView, StringListModel};
mod table_view;
pub use table_view::{StringTableModel, TableView};
mod tree_view;
pub use tree_view::{TreeModel, TreeView};
pub(crate) mod dialogs;
pub use dialogs::{FileDialog, MessageBox, MessageBoxButtons, MessageBoxResult};
mod settings;
pub use settings::Settings;

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
            /// Show or hide the widget.
            pub fn set_visible(&self, visible: bool) {
                if self.inner.is_alive() {
                    unsafe { ffi::widget_set_visible(self.inner.raw(), visible) };
                }
            }

            /// Bind the visibility to a signal. The binding lives in the
            /// component's owner and is released when the widget is destroyed.
            pub fn bind_visible(&self, signal: &Signal<bool>) {
                let weak = Rc::downgrade(&self.inner);
                let subscription = signal.observe(move |visible| {
                    if let Some(this) = weak.upgrade() {
                        this.set_widget_visible(*visible);
                    }
                });
                self.inner.owner.borrow_mut().add(subscription);
            }

            /// Whether the widget is currently visible on screen.
            pub fn is_visible(&self) -> bool {
                unsafe { ffi::widget_is_visible(self.inner.raw()) }
            }

            /// The widget's current width in pixels.
            pub fn width(&self) -> i32 {
                unsafe { ffi::widget_width(self.inner.raw()) }
            }

            /// The widget's current height in pixels.
            pub fn height(&self) -> i32 {
                unsafe { ffi::widget_height(self.inner.raw()) }
            }

            /// Escape hatch: the underlying Qt `QWidget` pointer.
            pub fn qobject_ptr(&self) -> *mut Void {
                self.inner.raw() as *mut Void
            }

            /// Set an application-defined style class for scoped Qt style-sheet rules.
            ///
            /// This leaves the platform style untouched unless the application supplies
            /// matching rules through [`Application::set_style_sheet`].
            pub fn set_style_class(&self, style_class: impl AsRef<str>) {
                if self.inner.is_alive() {
                    unsafe { ffi::widget_set_style_class(self.inner.raw(), style_class.as_ref()) };
                }
            }
        }

        impl IntoWidget for $name {
            fn component(&self) -> &Rc<Component> {
                &self.inner
            }
        }
    };
}

macro_rules! date_value_bindings {
    ($name:ident) => {
        impl $name {
            /// A stream of user value changes carrying the ISO text value.
            pub fn value_changed(&self) -> EventStream<String> {
                if self.inner.date_sink.borrow().is_none() {
                    unsafe {
                        ffi::widget_set_date_value_changed_cb(
                            self.inner.raw(),
                            &*self.inner as *const Component as *mut Void,
                        );
                    }
                    *self.inner.date_sink.borrow_mut() = Some(Rc::new(Sink::new()));
                }
                self.inner.date_sink.borrow().as_ref().unwrap().stream()
            }

            /// Bind the value to a signal. The binding lives in the widget's
            /// owner and is released with the widget tree.
            pub fn bind_value(&self, signal: &Signal<String>) {
                let weak = Rc::downgrade(&self.inner);
                let subscription = signal.observe(move |value| {
                    if let Some(this) = weak.upgrade() {
                        this.set_date_value((*value).clone());
                    }
                });
                self.inner.owner.borrow_mut().add(subscription);
            }

            /// Two-way bind the value to a `Var`, like Laminar's controlled
            /// input pattern.
            pub fn bind_value_two_way(&self, var: &Var<String>) {
                self.bind_value(&var.signal());
                let var = var.clone();
                let weak = Rc::downgrade(&self.inner);
                let subscription = self.value_changed().observe(move |value| {
                    if weak.upgrade().is_some() {
                        var.set((*value).clone());
                    }
                });
                self.inner.owner.borrow_mut().add(subscription);
            }
        }
    };
}

macro_rules! value_two_way {
    ($name:ident) => {
        impl $name {
            /// Two-way bind the value to a `Var`, like Laminar's controlled
            /// input pattern.
            pub fn bind_value_two_way(&self, var: &Var<i32>) {
                self.bind_value(&var.signal());
                let var = var.clone();
                let weak = Rc::downgrade(&self.inner);
                let subscription = self.value_changed().observe(move |value| {
                    if weak.upgrade().is_some() {
                        var.set(*value);
                    }
                });
                self.inner.owner.borrow_mut().add(subscription);
            }
        }
    };
}

use crate::bridge as ffi;
use crate::component::Component;
use std::cell::RefCell;
use std::rc::Rc;
use yse_model::Sink;

pub(crate) struct SelectionBridge {
    pub(crate) view: *mut ffi::Widget,
    pub(crate) sink: RefCell<Option<Rc<Sink<Vec<usize>>>>>,
    pub(crate) created_on: std::thread::ThreadId,
}

pub(crate) use date_value_bindings;
pub(crate) use value_two_way;
pub(crate) use widget_wrapper;
