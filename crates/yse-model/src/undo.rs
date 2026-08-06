//! Command-based undo/redo with reactive notifications.

use crate::{EventStream, Signal, Sink, Var};
use std::cell::RefCell;

/// A reversible operation managed by an [`UndoStack`].
pub trait Command {
    /// Apply the command's effect (also used for redo).
    fn execute(&self);

    /// Reverse the command's effect.
    fn undo(&self);

    /// Human-readable description, used for menu labels.
    fn text(&self) -> String {
        String::new()
    }
}

/// A stack of reversible commands with reactive `can_undo`/`can_redo` state
/// and a change stream.
pub struct UndoStack {
    undo: RefCell<Vec<Box<dyn Command>>>,
    redo: RefCell<Vec<Box<dyn Command>>>,
    can_undo: Var<bool>,
    can_redo: Var<bool>,
    changed: Sink<()>,
}

impl UndoStack {
    /// Create an empty stack.
    pub fn new() -> Self {
        Self {
            undo: RefCell::new(Vec::new()),
            redo: RefCell::new(Vec::new()),
            can_undo: Var::new(false),
            can_redo: Var::new(false),
            changed: Sink::new(),
        }
    }

    /// Execute `command` and push it onto the undo stack, clearing redo.
    pub fn push(&self, command: Box<dyn Command>) {
        command.execute();
        self.undo.borrow_mut().push(command);
        self.redo.borrow_mut().clear();
        self.notify();
    }

    /// Undo the most recent command; returns whether anything was undone.
    pub fn undo(&self) -> bool {
        let Some(command) = self.undo.borrow_mut().pop() else {
            return false;
        };
        command.undo();
        self.redo.borrow_mut().push(command);
        self.notify();
        true
    }

    /// Redo the most recently undone command; returns whether anything was
    /// redone.
    pub fn redo(&self) -> bool {
        let Some(command) = self.redo.borrow_mut().pop() else {
            return false;
        };
        command.execute();
        self.undo.borrow_mut().push(command);
        self.notify();
        true
    }

    /// Drop every command; no-op when the stack is already empty.
    pub fn clear(&self) {
        if self.undo.borrow().is_empty() && self.redo.borrow().is_empty() {
            return;
        }
        self.undo.borrow_mut().clear();
        self.redo.borrow_mut().clear();
        self.notify();
    }

    /// Whether an undo is available.
    pub fn can_undo(&self) -> Signal<bool> {
        self.can_undo.signal()
    }

    /// Whether a redo is available.
    pub fn can_redo(&self) -> Signal<bool> {
        self.can_redo.signal()
    }

    /// A stream of events after every push, undo, redo, or clear.
    pub fn changed(&self) -> EventStream<()> {
        self.changed.stream()
    }

    /// The text of the command that would be undone, if any.
    pub fn undo_text(&self) -> Option<String> {
        self.undo.borrow().last().map(|command| command.text())
    }

    /// The text of the command that would be redone, if any.
    pub fn redo_text(&self) -> Option<String> {
        self.redo.borrow().last().map(|command| command.text())
    }

    fn notify(&self) {
        self.can_undo.set(!self.undo.borrow().is_empty());
        self.can_redo.set(!self.redo.borrow().is_empty());
        self.changed.send(());
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}
