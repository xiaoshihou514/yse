//! Non-modal message box and file dialog wrappers.

use crate::bridge as ffi;
use crate::bridge::Void;
use crate::component::{Component, RetainedId, log_off_thread};
use crate::widgets::Window;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use yse_model::{EventStream, Sink};

pub enum MessageBoxButtons {
    Ok,
    OkCancel,
    YesNo,
}

/// The button a [`MessageBox`] was closed with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoxResult {
    Ok,
    Cancel,
    Yes,
    No,
}

const QMESSAGEBOX_OK: i32 = 0x0000_0400;
const QMESSAGEBOX_CANCEL: i32 = 0x0040_0000;
const QMESSAGEBOX_YES: i32 = 0x0000_4000;
const QMESSAGEBOX_NO: i32 = 0x0000_8000;
const QDIALOG_REJECTED: i32 = 0;
const QDIALOG_ACCEPTED: i32 = 1;

fn map_dialog_result(value: i32) -> MessageBoxResult {
    match value {
        QMESSAGEBOX_OK | QDIALOG_ACCEPTED => MessageBoxResult::Ok,
        QMESSAGEBOX_CANCEL | QDIALOG_REJECTED => MessageBoxResult::Cancel,
        QMESSAGEBOX_YES => MessageBoxResult::Yes,
        QMESSAGEBOX_NO => MessageBoxResult::No,
        _ => MessageBoxResult::Ok,
    }
}

pub(crate) struct DialogState {
    ptr: *mut ffi::Dialog,
    result_sink: RefCell<Option<Rc<Sink<MessageBoxResult>>>>,
    finished: Cell<bool>,
    created_on: std::thread::ThreadId,
    parent: std::rc::Weak<Component>,
    self_weak: std::rc::Weak<DialogState>,
    retained_id: Cell<Option<RetainedId>>,
}

impl Drop for DialogState {
    fn drop(&mut self) {
        unsafe { ffi::dialog_drop(self.ptr) };
    }
}

impl DialogState {
    pub(crate) fn on_finished(&self) {
        if self.finished.get() {
            return;
        }
        if std::thread::current().id() != self.created_on {
            log_off_thread("dialog finished");
        }
        let keep_alive = self.self_weak.upgrade();
        self.finished.set(true);
        let result = map_dialog_result(unsafe { ffi::dialog_last_result(self.ptr) });
        if let Some(sink) = self.result_sink.borrow().as_ref() {
            sink.send(result);
        }
        if let (Some(parent), Some(id)) = (self.parent.upgrade(), self.retained_id.take()) {
            parent.release(id);
        }
        drop(keep_alive);
    }
}

/// A non-modal message box whose result arrives on a stream.
pub struct MessageBox {
    inner: Rc<DialogState>,
}

impl Clone for MessageBox {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl MessageBox {
    /// Create a message box parented to `window`.
    pub fn new(
        window: &Window,
        title: impl Into<String>,
        text: impl Into<String>,
        buttons: MessageBoxButtons,
    ) -> Self {
        let buttons = match buttons {
            MessageBoxButtons::Ok => 0,
            MessageBoxButtons::OkCancel => 1,
            MessageBoxButtons::YesNo => 2,
        };
        let ptr = unsafe {
            ffi::dialog_message_new(window.inner.raw(), &title.into(), &text.into(), buttons)
        };
        let state = Rc::new_cyclic(|self_weak| DialogState {
            ptr,
            result_sink: RefCell::new(None),
            finished: Cell::new(false),
            created_on: std::thread::current().id(),
            parent: Rc::downgrade(&window.inner),
            self_weak: self_weak.clone(),
            retained_id: Cell::new(None),
        });
        state
            .retained_id
            .set(Some(window.inner.retain(state.clone())));
        unsafe {
            ffi::dialog_set_finished_cb(ptr, &*state as *const DialogState as *mut Void);
        }
        Self { inner: state }
    }

    /// Show the box (non-modal).
    pub fn show(&self) {
        unsafe { ffi::dialog_show(self.inner.ptr) };
    }

    /// Programmatically click the default button (used by tests).
    pub fn accept(&self) {
        unsafe { ffi::dialog_accept(self.inner.ptr) };
    }

    /// Close the box without choosing a button.
    pub fn close(&self) {
        unsafe { ffi::dialog_close(self.inner.ptr) };
    }

    /// A stream that emits the closing result exactly once.
    pub fn result(&self) -> EventStream<MessageBoxResult> {
        if self.inner.result_sink.borrow().is_none() {
            *self.inner.result_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.result_sink.borrow().as_ref().unwrap().stream()
    }
}

pub(crate) struct FileDialogState {
    ptr: *mut ffi::FileDialog,
    result_sink: RefCell<Option<Rc<Sink<Option<String>>>>>,
    finished: Cell<bool>,
    created_on: std::thread::ThreadId,
    parent: std::rc::Weak<Component>,
    self_weak: std::rc::Weak<FileDialogState>,
    retained_id: Cell<Option<RetainedId>>,
}

impl Drop for FileDialogState {
    fn drop(&mut self) {
        unsafe { ffi::filedialog_drop(self.ptr) };
    }
}

impl FileDialogState {
    pub(crate) fn on_finished(&self) {
        if self.finished.get() {
            return;
        }
        if std::thread::current().id() != self.created_on {
            log_off_thread("file dialog finished");
        }
        let keep_alive = self.self_weak.upgrade();
        self.finished.set(true);
        let selected = if unsafe { ffi::filedialog_last_result(self.ptr) } == QDIALOG_ACCEPTED {
            let path = unsafe { ffi::filedialog_selected_file(self.ptr) };
            (!path.is_empty()).then_some(path)
        } else {
            None
        };
        if let Some(sink) = self.result_sink.borrow().as_ref() {
            sink.send(selected);
        }
        if let (Some(parent), Some(id)) = (self.parent.upgrade(), self.retained_id.take()) {
            parent.release(id);
        }
        drop(keep_alive);
    }
}

/// A non-modal "open file" dialog whose selection arrives on a stream.
pub struct FileDialog {
    inner: Rc<FileDialogState>,
}

impl Clone for FileDialog {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl FileDialog {
    /// Create an open-file dialog parented to `window`.
    pub fn open(window: &Window, title: impl Into<String>) -> Self {
        let ptr = unsafe { ffi::filedialog_open_new(window.inner.raw(), &title.into()) };
        let state = Rc::new_cyclic(|self_weak| FileDialogState {
            ptr,
            result_sink: RefCell::new(None),
            finished: Cell::new(false),
            created_on: std::thread::current().id(),
            parent: Rc::downgrade(&window.inner),
            self_weak: self_weak.clone(),
            retained_id: Cell::new(None),
        });
        state
            .retained_id
            .set(Some(window.inner.retain(state.clone())));
        unsafe {
            ffi::filedialog_set_finished_cb(ptr, &*state as *const FileDialogState as *mut Void);
        }
        Self { inner: state }
    }

    /// Create a native folder-selection dialog parented to `window`.
    pub fn directory(window: &Window, title: impl Into<String>) -> Self {
        let ptr = unsafe { ffi::filedialog_directory_new(window.inner.raw(), &title.into()) };
        let state = Rc::new_cyclic(|self_weak| FileDialogState {
            ptr,
            result_sink: RefCell::new(None),
            finished: Cell::new(false),
            created_on: std::thread::current().id(),
            parent: Rc::downgrade(&window.inner),
            self_weak: self_weak.clone(),
            retained_id: Cell::new(None),
        });
        state
            .retained_id
            .set(Some(window.inner.retain(state.clone())));
        unsafe {
            ffi::filedialog_set_finished_cb(ptr, &*state as *const FileDialogState as *mut Void);
        }
        Self { inner: state }
    }

    /// Show the dialog (non-modal).
    pub fn show(&self) {
        unsafe { ffi::filedialog_show(self.inner.ptr) };
    }

    /// Programmatically accept (native dialogs may require an event loop).
    pub fn accept(&self) {
        unsafe { ffi::filedialog_accept(self.inner.ptr) };
    }

    /// Programmatically close without selecting (headless-friendly).
    pub fn close(&self) {
        unsafe { ffi::filedialog_close(self.inner.ptr) };
    }

    /// A stream that emits the selected path (or `None` if nothing was
    /// chosen) exactly once.
    pub fn result(&self) -> EventStream<Option<String>> {
        if self.inner.result_sink.borrow().is_none() {
            *self.inner.result_sink.borrow_mut() = Some(Rc::new(Sink::new()));
        }
        self.inner.result_sink.borrow().as_ref().unwrap().stream()
    }
}
