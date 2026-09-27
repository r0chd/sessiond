use std::{cell, ffi};

#[derive(Default)]
pub(crate) struct Logger {
    callback: cell::Cell<crate::LibsessionLogFn>,
    level: cell::Cell<crate::LibsessionLogLevel>,
}

impl Logger {
    pub(crate) fn set_callback(&self, callback: crate::LibsessionLogFn) {
        self.callback.set(callback);
    }

    pub(crate) fn set_level(&self, level: crate::LibsessionLogLevel) {
        self.level.set(level);
    }

    pub(crate) fn level(&self) -> crate::LibsessionLogLevel {
        self.level.get()
    }

    pub(crate) fn log(&self, level: crate::LibsessionLogLevel, message: impl FnOnce() -> String) {
        if level == crate::LibsessionLogLevel::Off || level > self.level.get() {
            return;
        }
        let Some(callback) = self.callback.get() else {
            return;
        };
        let Ok(message) = ffi::CString::new(message().replace('\0', "\\0")) else {
            return;
        };
        unsafe { callback(level, message.as_ptr()) };
    }
}
