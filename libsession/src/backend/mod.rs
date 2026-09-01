#[cfg(feature = "consolekit")]
pub mod consolekit;
#[cfg(feature = "logind")]
pub mod logind;
#[cfg(feature = "sessiond")]
pub mod sessiond;

pub trait Backend {
    fn get_session_id(&mut self) -> Option<String>;
}
