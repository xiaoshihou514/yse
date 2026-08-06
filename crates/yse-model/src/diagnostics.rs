//! Structured logging and reactive-graph diagnostics.
//!
//! Diagnostics answer "why did this change?": every transaction reports which
//! nodes were processed, which observers fired, whether writes were deferred,
//! and whether a cycle was rejected. Events are buffered during propagation
//! (so they never re-enter the graph mid-transaction) and flushed as a stream
//! when the transaction completes.

use crate::{EventStream, Sink};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A transactional graph event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticEvent {
    /// A transaction began.
    TransactionStarted { id: u64 },
    /// A node was recomputed / processed.
    NodeProcessed {
        node: u64,
        rank: usize,
        signal: bool,
    },
    /// An observer callback was delivered a value.
    ObserverFired { observer: u64 },
    /// A write performed during propagation was queued for the next pass.
    DeferredWriteQueued { transaction: u64 },
    /// A reactive cycle was detected and rejected.
    CycleRejected,
    /// A transaction finished, with summary counts.
    TransactionEnded {
        id: u64,
        processed: usize,
        fired: usize,
        deferred: usize,
    },
}

thread_local! {
    static DIAGNOSTICS: RefCell<Option<Rc<Diagnostics>>> = const { RefCell::new(None) };
    static FLUSHING: Cell<bool> = const { Cell::new(false) };
}

/// Installed diagnostics for the current thread.
pub struct Diagnostics {
    buffer: RefCell<Vec<DiagnosticEvent>>,
    sink: Sink<DiagnosticEvent>,
}

impl Diagnostics {
    /// Install diagnostics for the current thread. Panics if already
    /// installed.
    pub fn install() -> Rc<Self> {
        assert!(
            DIAGNOSTICS.with(|d| d.borrow().is_none()),
            "diagnostics already installed on this thread"
        );
        let diagnostics = Rc::new(Self {
            buffer: RefCell::new(Vec::new()),
            sink: Sink::new(),
        });
        DIAGNOSTICS.with(|d| *d.borrow_mut() = Some(diagnostics.clone()));
        diagnostics
    }

    /// Remove this installation from the thread, if it is the current one.
    pub fn uninstall(&self) {
        let is_current = DIAGNOSTICS.with(|d| {
            d.borrow()
                .as_ref()
                .is_some_and(|current| std::ptr::eq(Rc::as_ptr(current), self))
        });
        if is_current {
            DIAGNOSTICS.with(|d| *d.borrow_mut() = None);
        }
    }

    /// A stream of diagnostic events, flushed after each transaction.
    pub fn events(&self) -> EventStream<DiagnosticEvent> {
        self.sink.stream()
    }

    /// Deliver any buffered events now (also done automatically at the end of
    /// every transaction).
    pub fn flush(&self) {
        if FLUSHING.get() {
            return;
        }
        FLUSHING.set(true);
        let events: Vec<DiagnosticEvent> = std::mem::take(&mut *self.buffer.borrow_mut());
        for event in events {
            self.sink.send(event);
        }
        FLUSHING.set(false);
    }

    pub(crate) fn emit(event: DiagnosticEvent) {
        if FLUSHING.get() {
            return;
        }
        DIAGNOSTICS.with(|d| {
            if let Some(diagnostics) = d.borrow().as_ref() {
                diagnostics.buffer.borrow_mut().push(event);
            }
        });
    }

    pub(crate) fn flush_global() {
        DIAGNOSTICS.with(|d| {
            if let Some(diagnostics) = d.borrow().as_ref() {
                diagnostics.flush();
            }
        });
    }
}

/// Severity of a log record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// A structured log record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
}

thread_local! {
    static LOGGER: RefCell<Option<Rc<Logger>>> = const { RefCell::new(None) };
}

/// A leveled logger whose records are observable as a stream.
pub struct Logger {
    min_level: Cell<LogLevel>,
    sink: Sink<LogRecord>,
}

impl Logger {
    /// Install a logger for the current thread. Panics if already installed.
    pub fn install(min_level: LogLevel) -> Rc<Self> {
        assert!(
            LOGGER.with(|l| l.borrow().is_none()),
            "logger already installed on this thread"
        );
        let logger = Rc::new(Self {
            min_level: Cell::new(min_level),
            sink: Sink::new(),
        });
        LOGGER.with(|l| *l.borrow_mut() = Some(logger.clone()));
        logger
    }

    /// Remove this installation from the thread, if it is the current one.
    pub fn uninstall(&self) {
        let is_current = LOGGER.with(|l| {
            l.borrow()
                .as_ref()
                .is_some_and(|current| std::ptr::eq(Rc::as_ptr(current), self))
        });
        if is_current {
            LOGGER.with(|l| *l.borrow_mut() = None);
        }
    }

    /// A stream of log records at or above the current minimum level.
    pub fn records(&self) -> EventStream<LogRecord> {
        self.sink.stream()
    }

    /// Set the minimum level that is emitted.
    pub fn set_min_level(&self, level: LogLevel) {
        self.min_level.set(level);
    }

    /// Log `message` at `level` if it meets the minimum level.
    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        if level <= self.min_level.get() {
            self.sink.send(LogRecord {
                level,
                message: message.into(),
            });
        }
    }

    pub fn debug(&self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    pub fn info(&self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    pub fn warn(&self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    pub fn error(&self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }
}

/// Log through the thread's installed logger; no-op when none is installed.
pub fn log(level: LogLevel, message: impl Into<String>) {
    LOGGER.with(|l| {
        if let Some(logger) = l.borrow().as_ref() {
            logger.log(level, message);
        }
    });
}

pub fn log_debug(message: impl Into<String>) {
    log(LogLevel::Debug, message);
}

pub fn log_info(message: impl Into<String>) {
    log(LogLevel::Info, message);
}

pub fn log_warn(message: impl Into<String>) {
    log(LogLevel::Warn, message);
}

pub fn log_error(message: impl Into<String>) {
    log(LogLevel::Error, message);
}
