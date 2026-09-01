#![allow(non_camel_case_types)]

use std::ffi;

#[repr(C)]
pub struct libsession_context {
    _private: [u8; 0],
}

#[repr(C)]
pub struct libsession_session_id {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn libsession_new() -> *mut libsession_context;
    pub fn libsession_free(ctx: *mut libsession_context);

    pub fn libsession_get_session_id(ctx: *mut libsession_context) -> *mut libsession_session_id;
    pub fn libsession_session_id_str(id: *const libsession_session_id) -> *const ffi::c_char;
    pub fn libsession_session_id_free(id: *mut libsession_session_id);
}
