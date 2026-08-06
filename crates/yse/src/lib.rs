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
