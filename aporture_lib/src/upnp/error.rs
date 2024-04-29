use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not find local ip address")]
    LocalIpNotFound,
    #[error("Could not find upnp enabled gateway")]
    GatewayNotFound,
    #[error("Could not operate upnp gateway to open port")]
    OpenPort,
    #[error("Last port was already closed or never opened")]
    ClosePort,
    #[error("Could not perform operation on gateway")]
    UPnP,
}

impl From<local_ip_address::Error> for Error {
    fn from(_: local_ip_address::Error) -> Self {
        Self::LocalIpNotFound
    }
}

impl From<igd::SearchError> for Error {
    fn from(_: igd::SearchError) -> Self {
        Self::GatewayNotFound
    }
}

impl From<igd::AddAnyPortError> for Error {
    fn from(_: igd::AddAnyPortError) -> Self {
        Self::OpenPort
    }
}

impl From<igd::RemovePortError> for Error {
    fn from(value: igd::RemovePortError) -> Self {
        match value {
            igd::RemovePortError::NoSuchPortMapping => Self::ClosePort,
            _ => Self::UPnP,
        }
    }
}
