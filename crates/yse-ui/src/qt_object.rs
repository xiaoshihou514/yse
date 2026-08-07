//! CXX-Qt objects used as the Qt-facing half of Yse's reactive bindings.
//!
//! Widget construction remains in the small C++ Widgets shim, while values
//! that Qt observes are real `QObject` properties.  This gives Qt native
//! change notification and keeps Rust application state out of hand-written
//! callback trampolines.

use cxx_qt_lib::QString;

#[cxx_qt::bridge(namespace = "yse_ui")]
pub mod qobject {
    #[namespace = ""]
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        /// A QObject property backing text displayed by a Qt widget.
        #[qobject]
        #[qproperty(QString, text)]
        type TextState = super::TextStateRust;

        /// A QObject property backing a checkable Qt widget.
        #[qobject]
        #[qproperty(bool, checked)]
        type ToggleState = super::ToggleStateRust;

        /// A QObject property set backing the mutable part of a QAction.
        #[qobject]
        #[qproperty(QString, text)]
        #[qproperty(bool, enabled)]
        type ActionState = super::QtActionStateRust;

        /// Common QWidget presentation properties shared by all wrappers.
        #[qobject]
        #[qproperty(bool, enabled)]
        #[qproperty(bool, visible)]
        #[qproperty(QString, title)]
        type WidgetState = super::WidgetStateRust;
    }
}

/// Rust-owned state for [`qobject::TextState`].
#[derive(Default)]
pub struct TextStateRust {
    pub text: QString,
}

/// Rust-owned state for [`qobject::ToggleState`].
#[derive(Default)]
pub struct ToggleStateRust {
    pub checked: bool,
}

/// Rust-owned properties for [`qobject::ActionState`].
pub struct QtActionStateRust {
    pub text: QString,
    pub enabled: bool,
}

impl Default for QtActionStateRust {
    fn default() -> Self {
        Self {
            text: QString::default(),
            enabled: true,
        }
    }
}

/// Rust-owned presentation properties for [`qobject::WidgetState`].
pub struct WidgetStateRust {
    pub enabled: bool,
    pub visible: bool,
    pub title: QString,
}

impl Default for WidgetStateRust {
    fn default() -> Self {
        Self {
            enabled: true,
            visible: false,
            title: QString::default(),
        }
    }
}
