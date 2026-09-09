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

#[repr(C)]
pub enum LibsessionStatus {
    Ok = 0,
    Unsupported = 1,
    Failed = 2,
}

#[repr(C)]
pub enum LibsessionCapability {
    Unavailable = 0,
    Available = 1,
    Unsupported = 2,
    Failed = 3,
}

unsafe extern "C" {
    pub fn libsession_new() -> *mut libsession_context;
    pub fn libsession_free(ctx: *mut libsession_context);

    pub fn libsession_get_session_id(ctx: *mut libsession_context) -> *mut libsession_session_id;
    pub fn libsession_session_id_str(id: *const libsession_session_id) -> *const ffi::c_char;
    pub fn libsession_session_id_free(id: *mut libsession_session_id);

    pub fn libsession_power_off(ctx: *mut libsession_context) -> LibsessionStatus;
    pub fn libsession_reboot(ctx: *mut libsession_context) -> LibsessionStatus;
    pub fn libsession_suspend(ctx: *mut libsession_context) -> LibsessionStatus;
    pub fn libsession_hibernate(ctx: *mut libsession_context) -> LibsessionStatus;
    pub fn libsession_hybrid_sleep(ctx: *mut libsession_context) -> LibsessionStatus;

    pub fn libsession_can_power_off(ctx: *mut libsession_context) -> LibsessionCapability;
    pub fn libsession_can_reboot(ctx: *mut libsession_context) -> LibsessionCapability;
    pub fn libsession_can_suspend(ctx: *mut libsession_context) -> LibsessionCapability;
    pub fn libsession_can_hibernate(ctx: *mut libsession_context) -> LibsessionCapability;
    pub fn libsession_can_hybrid_sleep(ctx: *mut libsession_context) -> LibsessionCapability;
}
