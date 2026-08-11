//! QSettings-backed key/value store wrapper.

use crate::bridge as ffi;
use std::rc::Rc;

/// A cloneable handle to an application-scoped Qt settings store.
pub struct Settings {
    inner: Rc<SettingsState>,
}

struct SettingsState {
    ptr: *mut ffi::Settings,
}

impl Clone for Settings {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Settings {
    /// Open the application configuration store for `organization` and
    /// `application`.
    pub fn new(organization: impl Into<String>, application: impl Into<String>) -> Self {
        Self {
            inner: Rc::new(SettingsState {
                ptr: unsafe { ffi::settings_new(&organization.into(), &application.into()) },
            }),
        }
    }

    /// The value for `key`, if set.
    pub fn value(&self, key: &str) -> Option<String> {
        if !self.contains(key) {
            return None;
        }
        Some(unsafe { ffi::settings_value(self.inner.ptr, key) })
    }

    /// Set `key` to `value`.
    pub fn set(&self, key: &str, value: &str) {
        unsafe { ffi::settings_set(self.inner.ptr, key, value) };
    }

    /// Remove `key`.
    pub fn remove(&self, key: &str) {
        unsafe { ffi::settings_remove(self.inner.ptr, key) };
    }

    /// Whether `key` is present.
    pub fn contains(&self, key: &str) -> bool {
        unsafe { ffi::settings_contains(self.inner.ptr, key) }
    }

    /// Flush pending writes to disk.
    pub fn sync(&self) {
        unsafe { ffi::settings_sync(self.inner.ptr) };
    }
}

impl Drop for SettingsState {
    fn drop(&mut self) {
        unsafe { ffi::settings_drop(self.ptr) };
    }
}
