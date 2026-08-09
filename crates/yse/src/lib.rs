//! Yse: build and ship reactive Qt desktop applications from Rust.
//!
//! This is the convenience facade; it re-exports the public API of
//! [`yse_model`] (the pure-Rust reactive runtime) and [`yse_ui`] (the Qt
//! Widgets layer).
//!
//! ```
//! use yse::{transaction, ListModel, Var};
//!
//! let value = Var::new(0);
//! let doubled = value.signal().map(|v| v * 2);
//! transaction(|| value.set(21));
//! assert_eq!(*doubled.value(), 42);
//!
//! let list = ListModel::from(vec![1, 2, 3]);
//! assert_eq!(list.len(), 3);
//! ```

pub use yse_model::*;
pub use yse_ui::*;

/// Common application types and functions.
///
/// Import this module when an application prefers an explicit, stable starting
/// surface over the facade's complete re-exports.
pub mod prelude {
    pub use yse_model::{
        EventStream, ListChange, ListModel, Owner, QueueScheduler, Scheduler, Signal, Sink,
        Subscription, UndoStack, Var, spawn_task, transaction,
    };
    pub use yse_ui::{
        Action, Application, Button, CheckBox, Column, ComboBox, FileDialog, Grid, Label, LineEdit,
        ListView, Menu, MenuBar, MessageBox, ProgressBar, Row, Settings, Slider, SpinBox,
        StringListModel, StringTableModel, TableView, ToolBar, Window,
    };
}
