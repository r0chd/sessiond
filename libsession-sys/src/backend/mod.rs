#[cfg(feature = "consolekit")]
pub mod consolekit;
#[cfg(feature = "logind")]
pub mod logind;
#[cfg(feature = "sessiond")]
pub mod sessiond;

use std::ffi;

pub trait Backend {
    fn get_owner_uid(&mut self, pid: u32) -> Option<u32>;

    fn get_display(&mut self, uid: u32) -> Option<String>;

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState>;

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32>;

    fn get_session_id(&mut self, pid: u32) -> Option<String>;

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState>;
}
