use libsystemd_sys::login;
use std::{ffi, ptr};

fn sd_string(value: *mut ffi::c_char) -> Option<String> {
    let value = ptr::NonNull::new(value)?;
    let result = unsafe { ffi::CStr::from_ptr(value.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(value.as_ptr().cast()) };
    Some(result)
}
pub struct Logind {}

impl Logind {
    pub fn try_connect() -> Option<Self> {
        let mut sessions = ptr::null_mut::<*mut ffi::c_char>();
        let result = unsafe { login::sd_get_sessions(&raw mut sessions) };
        if result < 0 {
            return None;
        }
        if !sessions.is_null() {
            for index in 0..usize::try_from(result).ok()? {
                unsafe {
                    libc::free((*sessions.add(index)).cast());
                }
            }
            unsafe { libc::free(sessions.cast()) };
        }
        Some(Self {})
    }
}

impl super::Backend for Logind {
    fn is_local_session(&mut self, session_id: &ffi::CStr) -> Option<bool> {
        let result = unsafe { login::sd_session_is_remote(session_id.as_ptr()) };
        (result >= 0).then_some(result == 0)
    }

    fn get_owner_uid(&mut self, pid: u32) -> Option<u32> {
        let mut uid = 0;
        let result = unsafe { login::sd_pid_get_owner_uid(pid.cast_signed(), &raw mut uid) };
        (result >= 0).then_some(uid)
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        let mut state = ptr::null_mut();
        let result = unsafe { login::sd_uid_get_state(uid, &raw mut state) };
        let state = (result >= 0).then(|| sd_string(state))??;
        Some(match state.as_str() {
            "offline" => crate::LibsessionUserState::Offline,
            "lingering" => crate::LibsessionUserState::Lingering,
            "online" => crate::LibsessionUserState::Online,
            "active" => crate::LibsessionUserState::Active,
            "closing" => crate::LibsessionUserState::Closing,
            _ => return None,
        })
    }

    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        let mut session = ptr::null_mut();
        let result = unsafe { login::sd_pid_get_session(pid.cast_signed(), &raw mut session) };
        (result >= 0).then(|| sd_string(session))?
    }

    fn get_display(&mut self, uid: u32) -> Option<String> {
        let mut display = ptr::null_mut();
        let result = unsafe { login::sd_uid_get_display(uid, &raw mut display) };
        (result >= 0).then(|| sd_string(display))?
    }

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32> {
        let mut uid = 0;
        let result = unsafe { login::sd_session_get_uid(session_id.as_ptr(), &raw mut uid) };
        (result >= 0).then_some(uid)
    }

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState> {
        let mut state = ptr::null_mut();
        let result = unsafe { login::sd_session_get_state(session_id.as_ptr(), &raw mut state) };
        let state = (result >= 0).then(|| sd_string(state))??;
        Some(match state.as_str() {
            "online" => crate::LibsessionSessionState::Online,
            "active" => crate::LibsessionSessionState::Active,
            "closing" => crate::LibsessionSessionState::Closing,
            "opening" => crate::LibsessionSessionState::Opening,
            _ => crate::LibsessionSessionState::Unkown,
        })
    }
}
