//! Actions, menu bar, menus, and toolbars.

use crate::action::ActionState;
use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::widgets::{IntoWidget, widget_wrapper};
use std::rc::Rc;
use yse_model::{EventStream, Signal, Sink};

pub struct Action {
    inner: Rc<ActionState>,
}

impl Clone for Action {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Action {
    /// A stream of trigger events.
    pub fn triggered(&self) -> EventStream<()> {
        if self.inner.triggered_sink.borrow().is_none() {
            unsafe {
                ffi::action_set_triggered_cb(
                    self.inner.raw(),
                    &*self.inner as *const ActionState as *mut Void,
                );
            }
            *self.inner.triggered_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner
            .triggered_sink
            .borrow()
            .as_ref()
            .unwrap()
            .stream()
    }

    /// Register a trigger handler.
    pub fn on_trigger<F>(&self, f: F) -> &Self
    where
        F: FnMut(&()) + 'static,
    {
        self.inner
            .owner
            .borrow_mut()
            .add(self.triggered().observe(f));
        self
    }

    /// Enable or disable the action.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }

    /// Bind the enabled state to a signal.
    pub fn bind_enabled(&self, signal: &Signal<bool>) {
        let weak = Rc::downgrade(&self.inner);
        let subscription = signal.observe(move |enabled| {
            if let Some(this) = weak.upgrade() {
                this.set_enabled(*enabled);
            }
        });
        self.inner.owner.borrow_mut().add(subscription);
    }

    /// Set the action text.
    pub fn set_text(&self, text: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_text(self.inner.raw(), &text.into()) };
        }
    }

    /// Use a platform icon-theme name, e.g. `"document-open"` from Breeze.
    pub fn set_icon(&self, theme_name: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_icon(self.inner.raw(), &theme_name.into()) };
        }
    }

    /// Set an icon-theme name and return the action for chaining.
    pub fn icon(self, theme_name: impl Into<String>) -> Self {
        self.set_icon(theme_name);
        self
    }

    /// Set a keyboard shortcut, e.g. `"Ctrl+Q"`.
    pub fn set_shortcut(&self, shortcut: impl Into<String>) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_shortcut(self.inner.raw(), &shortcut.into()) };
        }
    }

    /// Set a keyboard shortcut and return the action, for chaining.
    pub fn shortcut(self, shortcut: impl Into<String>) -> Self {
        self.set_shortcut(shortcut);
        self
    }

    /// Make the action checkable (shows a check mark when toggled).
    pub fn set_checkable(&self, checkable: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_checkable(self.inner.raw(), checkable) };
        }
    }

    /// Set the checked state of a checkable action.
    pub fn set_checked(&self, checked: bool) {
        if self.inner.is_alive() {
            unsafe { ffi::action_set_checked(self.inner.raw(), checked) };
        }
    }

    /// The current checked state.
    pub fn checked(&self) -> bool {
        unsafe { ffi::action_checked(self.inner.raw()) }
    }

    /// Trigger the action programmatically (used by tests and smoke runs).
    pub fn trigger(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::action_trigger(self.inner.raw()) };
        }
    }
}

/// A menu bar attached to a window.
pub struct MenuBar {
    pub(crate) inner: Rc<Component>,
}

impl MenuBar {
    /// Create a menu in this bar.
    pub fn menu(&self, title: impl Into<String>) -> Menu {
        Menu {
            inner: unsafe {
                Component::from_raw_child(
                    ffi::menu_new(&title.into(), self.inner.raw()),
                    &self.inner,
                )
            },
        }
    }

    /// Create a menu and hand it to `f`; `f`'s value (typically action
    /// handles) is passed through so menus build inline.
    pub fn menu_with<R, F: FnOnce(&Menu) -> R>(&self, title: impl Into<String>, f: F) -> R {
        let menu = self.menu(title);
        f(&menu)
    }
}

widget_wrapper!(MenuBar);

/// A menu of actions.
pub struct Menu {
    pub(crate) inner: Rc<Component>,
}

impl Menu {
    /// Create an action in this menu.
    pub fn action(&self, text: impl Into<String>) -> Action {
        let inner =
            unsafe { ActionState::from_raw(ffi::action_new(&text.into(), self.inner.raw())) };
        self.inner.retain(inner.clone());
        Action { inner }
    }

    /// Add a separator.
    pub fn separator(&self) -> &Self {
        if self.inner.is_alive() {
            unsafe { ffi::menu_add_separator(self.inner.raw()) };
        }
        self
    }

    /// Open the menu as a popup at the current cursor position. The menu
    /// stays open until dismissed; keep a handle alive while it is shown.
    pub fn popup(&self) {
        if self.inner.is_alive() {
            unsafe { ffi::menu_popup(self.inner.raw()) };
        }
    }
}

widget_wrapper!(Menu);

/// A toolbar attached to a window.
pub struct ToolBar {
    pub(crate) inner: Rc<Component>,
}

impl ToolBar {
    /// Add an action to this toolbar.
    pub fn add(&self, action: &Action) {
        if self.inner.is_alive() {
            unsafe { ffi::toolbar_add_action(self.inner.raw(), action.inner.raw()) };
        }
    }
}

widget_wrapper!(ToolBar);
