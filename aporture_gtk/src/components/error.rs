use aporture::pairing::error;
use aporture::transfer::{ReceiveError, SendError};

#[derive(Debug)]
pub enum PeerError {
    NoPeer,
    NoServer,
    InvalidServer,
    ServerFailure,
    PairingFailure,
    FileNotFound,
    HashMismatch,
    TransferFailure,
}

impl From<error::Hello> for PeerError {
    fn from(e: error::Hello) -> Self {
        log::warn!("Error: {e}");

        match e {
            error::Hello::NoServer(_) => Self::NoServer,
            error::Hello::NoPeer => {
                log::warn!("Selected passphrase did not match a sender");
                Self::NoPeer
            }
            error::Hello::ServerUnsupportedVersion | error::Hello::ClientError => {
                Self::InvalidServer
            }
            error::Hello::ServerError(_) => Self::ServerFailure,
        }
    }
}

impl From<error::KeyExchange> for PeerError {
    fn from(e: error::KeyExchange) -> Self {
        log::warn!("Error: {e}");
        Self::PairingFailure
    }
}

impl From<error::Negotiation> for PeerError {
    fn from(e: error::Negotiation) -> Self {
        log::warn!("Error: {e}");
        Self::PairingFailure
    }
}

impl From<error::Error> for PeerError {
    fn from(e: error::Error) -> Self {
        e.into()
    }
}

impl From<ReceiveError> for PeerError {
    fn from(e: ReceiveError) -> Self {
        log::warn!("Error: {e}");

        match e {
            ReceiveError::File(_) => Self::FileNotFound,
            ReceiveError::Network(_) | ReceiveError::Cipher(_) => Self::TransferFailure,
            ReceiveError::HashMismatch => Self::HashMismatch,
        }
    }
}

impl From<SendError> for PeerError {
    fn from(e: SendError) -> Self {
        log::warn!("Error: {e}");

        match e {
            SendError::File(_) | SendError::Path => Self::FileNotFound,
            SendError::Network(_) => Self::TransferFailure,
            SendError::HashMismatch => Self::HashMismatch,
        }
    }
}
