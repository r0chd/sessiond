use crate::backend;
#[cfg(feature = "sessiond")]
use crate::sessiond_state;
use std::io;
#[cfg(feature = "consolekit")]
use zbus::DBusError;
#[cfg(feature = "consolekit")]
use zbus::fdo;

#[cfg(feature = "sessiond")]
impl From<sessiond_state::Error> for backend::Error {
    fn from(err: sessiond_state::Error) -> Self {
        match err {
            sessiond_state::Error::Io(err) => Self::Io(err),
            err @ sessiond_state::Error::Decode(_) => {
                Self::Io(io::Error::new(io::ErrorKind::InvalidData, err))
            }
            err => Self::Io(io::Error::other(err)),
        }
    }
}

impl From<&backend::Error> for crate::LibsessionStatus {
    fn from(error: &backend::Error) -> Self {
        match error {
            backend::Error::Io(error) => error.into(),
            backend::Error::ParseUid(_) => Self::InvalidData,
            #[cfg(feature = "consolekit")]
            backend::Error::Dbus(error) => error.into(),
        }
    }
}

impl From<&io::Error> for crate::LibsessionStatus {
    fn from(value: &io::Error) -> Self {
        if matches!(value.raw_os_error(), Some(libc::ESRCH | libc::ENODATA)) {
            return Self::NotFound;
        }

        match value.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::InvalidInput => Self::InvalidArgument,
            io::ErrorKind::InvalidData => Self::InvalidData,
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::NotConnected
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::TimedOut => Self::Unavailable,
            _ => Self::IoError,
        }
    }
}

#[cfg(feature = "consolekit")]
impl From<&zbus::Error> for crate::LibsessionStatus {
    fn from(value: &zbus::Error) -> Self {
        let name = match value {
            zbus::Error::InputOutput(error) => {
                return if error.kind() == io::ErrorKind::NotFound {
                    Self::Unavailable
                } else {
                    error.as_ref().into()
                };
            }
            zbus::Error::MethodError(name, _, _) => name.as_str(),
            zbus::Error::FDO(error) => {
                if let fdo::Error::ZBus(error) = error.as_ref() {
                    return error.into();
                }
                return dbus_name_status(error.name().as_str());
            }
            _ => return Self::DbusError,
        };
        dbus_name_status(name)
    }
}

#[cfg(feature = "consolekit")]
fn dbus_name_status(name: &str) -> crate::LibsessionStatus {
    match name {
        "org.freedesktop.DBus.Error.AccessDenied" | "org.freedesktop.DBus.Error.AuthFailed" => {
            crate::LibsessionStatus::PermissionDenied
        }
        "org.freedesktop.DBus.Error.InvalidArgs" => crate::LibsessionStatus::InvalidArgument,
        "org.freedesktop.DBus.Error.UnknownObject" => crate::LibsessionStatus::NotFound,
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.DBus.Error.NoServer"
        | "org.freedesktop.DBus.Error.NoNetwork"
        | "org.freedesktop.DBus.Error.Disconnected"
        | "org.freedesktop.DBus.Error.NoReply"
        | "org.freedesktop.DBus.Error.Timeout"
        | "org.freedesktop.DBus.Error.TimedOut" => crate::LibsessionStatus::Unavailable,
        _ => crate::LibsessionStatus::DbusError,
    }
}
