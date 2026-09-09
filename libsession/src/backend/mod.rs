#[cfg(feature = "consolekit")]
pub mod consolekit;
#[cfg(feature = "logind")]
pub mod logind;
#[cfg(feature = "sessiond")]
pub mod sessiond;

pub enum Error {
    Unsupported,
    Failed,
}

pub trait Backend {
    fn get_session_id(&mut self) -> Option<String>;

    fn power_off(&mut self) -> Result<(), Error>;

    fn can_power_off(&mut self) -> Result<bool, Error>;

    fn reboot(&mut self) -> Result<(), Error>;

    fn can_reboot(&mut self) -> Result<bool, Error>;

    fn suspend(&mut self) -> Result<(), Error>;

    fn can_suspend(&mut self) -> Result<bool, Error>;

    fn hibernate(&mut self) -> Result<(), Error>;

    fn can_hibernate(&mut self) -> Result<bool, Error>;

    fn hybrid_sleep(&mut self) -> Result<(), Error>;

    fn can_hybrid_sleep(&mut self) -> Result<bool, Error>;
}
