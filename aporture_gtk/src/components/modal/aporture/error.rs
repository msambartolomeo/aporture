use aporture::pairing::error::Error as PairingError;
use aporture::transfer::{ReceiveError, SendError};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("The peer sending the file has not arrived yet")]
    NoPeer,
    #[error("Could not connect to server")]
    NoServer,
    #[error("The server is malfunctioning, please try again later")]
    InvalidServer,
    #[error("The server is malfunctioning, please try again later")]
    ServerFailure,
    #[error("Could not perform pairing with peer")]
    PairingFailure,
    #[error("The file selected is invalid")]
    FileNotFound,
    #[error("You do not have access to the file you are trying to send")]
    FilePermission,
    #[error("There was a problem in the transfered file")]
    HashMismatch,
    #[error("Could not transfer file")]
    TransferFailure,
    #[error("Contact not found")]
    NoContact,
    #[error("Could not save the contact")]
    ContactSaving,
}

impl From<PairingError> for Error {
    fn from(e: PairingError) -> Self {
        log::error!("Error: {e}");

        match e {
            PairingError::Hello(e) => match e {
                aporture::pairing::error::Hello::NoServer(_) => Self::NoServer,
                aporture::pairing::error::Hello::NoPeer => {
                    log::warn!("Selected passphrase did not match a sender");
                    Self::NoPeer
                }
                aporture::pairing::error::Hello::ServerUnsupportedVersion
                | aporture::pairing::error::Hello::ClientError => Self::InvalidServer,
                aporture::pairing::error::Hello::ServerError(_) => Self::ServerFailure,
            },
            PairingError::KeyExchange(_) | PairingError::AddressExchange(_) => Self::PairingFailure,
        }
    }
}

impl From<ReceiveError> for Error {
    fn from(e: ReceiveError) -> Self {
        log::error!("Error: {e}");

        match e {
            ReceiveError::File(_) | ReceiveError::Destination => Self::FileNotFound,
            ReceiveError::Network(_) | ReceiveError::Cipher(_) => Self::TransferFailure,
            ReceiveError::HashMismatch => Self::HashMismatch,
        }
    }
}

impl From<SendError> for Error {
    fn from(e: SendError) -> Self {
        log::error!("Error: {e}");

        match e {
            SendError::File(_) | SendError::Path => Self::FileNotFound,
            SendError::Subpath(_) => Self::FilePermission,
            SendError::Network(_) => Self::TransferFailure,
            SendError::HashMismatch => Self::HashMismatch,
        }
    }
}
