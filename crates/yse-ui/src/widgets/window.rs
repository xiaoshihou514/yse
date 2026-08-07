//! Top-level window wrapper.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{Column, Grid, MenuBar, Row, ToolBar};
use std::rc::Rc;

pub struct Window {
    pub(crate) inner: Rc<Component>,
}

impl Clone for Window {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Window {
    /// Create a new window.
    pub fn new() -> Self {
        Self {
            inner: unsafe { Component::from_raw(ffi::widget_new_window()) },
        }
    }

    /// Show the window.
    pub fn show(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_show(self.inner.raw()) };
        }
    }

    /// Show or hide the window.
    pub fn set_visible(&self, visible: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_visible(self.inner.raw(), visible) };
        }
    }

    /// Set the window title.
    pub fn set_title(&self, title: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_set_title(self.inner.raw(), &title.into()) };
        }
    }

    /// Set the window size in pixels.
    pub fn set_size(&self, width: u32, height: u32) {
        if self.inner.is_alive() {
            unsafe { ffi::widget_resize(self.inner.raw(), width as i32, height as i32) };
        }
    }

    /// Create a horizontal row inside this window.
    pub fn row(&self) -> Row {
        Row {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_row(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a vertical column inside this window.
    pub fn column(&self) -> Column {
        Column {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_column(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a grid layout inside this window.
    pub fn grid(&self) -> Grid {
        let inner = unsafe {
            Component::from_raw_child(ffi::widget_new_grid(self.inner.raw()), &self.inner)
        };
        unsafe { ffi::layout_add(self.inner.raw(), inner.raw()) };
        Grid { inner }
    }

    /// Create a menu bar for this window.
    pub fn menu_bar(&self) -> MenuBar {
        MenuBar {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_menubar(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Create a toolbar for this window.
    pub fn toolbar(&self) -> ToolBar {
        ToolBar {
            inner: unsafe {
                Component::from_raw_child(ffi::widget_new_toolbar(self.inner.raw()), &self.inner)
            },
        }
    }

    /// Escape hatch: the underlying Qt `QWidget` pointer.
    pub fn qobject_ptr(&self) -> *mut Void {
        self.inner.raw() as *mut Void
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}
