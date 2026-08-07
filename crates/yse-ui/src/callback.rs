//! The single place where raw widget callbacks are turned back into safe
//! method calls on the owning component.

use crate::SelectionBridge;
use crate::action::ActionState;
use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::Component;
use crate::{DialogState, FileDialogState};

/// # Safety
///
/// `data` must be a pointer to a live `Component` that was handed to the C++
/// shim and whose widget has not been destroyed.
pub(crate) unsafe fn clicked(data: *mut Void) {
    unsafe {
        (*(data as *const Component)).on_clicked();
    }
}

/// # Safety
///
/// Same contract as [`clicked`].
pub(crate) unsafe fn text_changed(data: *mut Void) {
    unsafe {
        (*(data as *const Component)).on_text_changed();
    }
}

/// # Safety
///
/// Same contract as [`clicked`].
pub(crate) unsafe fn toggled(data: *mut Void) {
    unsafe {
        (*(data as *const Component)).on_toggled();
    }
}

/// # Safety
///
/// Same contract as [`clicked`].
pub(crate) unsafe fn value_changed(data: *mut Void) {
    unsafe {
        (*(data as *const Component)).on_value_changed();
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `Component`; it stays valid because the
/// Rust wrapper outlives the widget destruction that triggers this callback.
pub(crate) unsafe fn destroyed(data: *mut Void) {
    unsafe {
        (*(data as *const Component)).on_destroyed();
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `ActionState` handed to the shim.
pub(crate) unsafe fn action_triggered(data: *mut Void) {
    unsafe {
        (*(data as *const ActionState)).on_triggered();
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `ActionState`; it stays valid because
/// the Rust wrapper outlives the action destruction that triggers this
/// callback.
pub(crate) unsafe fn action_destroyed(data: *mut Void) {
    unsafe {
        (*(data as *const ActionState)).on_destroyed();
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `SelectionBridge` handed to the shim.
pub(crate) unsafe fn selection_changed(data: *mut Void) {
    unsafe {
        let bridge = &*(data as *const SelectionBridge);
        if std::thread::current().id() != bridge.created_on {
            crate::component::log_off_thread("selection changed");
        }
        let rows: Vec<usize> = ffi::view_selected_rows(bridge.view)
            .into_iter()
            .map(|row| row as usize)
            .collect();
        if let Some(sink) = bridge.sink.borrow().as_ref() {
            sink.send(rows);
        }
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `DialogState` handed to the shim.
pub(crate) unsafe fn dialog_finished(data: *mut Void) {
    unsafe {
        (*(data as *const DialogState)).on_finished();
    }
}

/// # Safety
///
/// `data` must be a pointer to a live `FileDialogState` handed to the shim.
pub(crate) unsafe fn filedialog_finished(data: *mut Void) {
    unsafe {
        (*(data as *const FileDialogState)).on_finished();
    }
}
