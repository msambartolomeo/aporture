use net::message;
#[cfg(feature = "full")]
use quinn::ConnectionError;
use thiserror::Error;

pub mod net;

#[cfg(feature = "full")]
pub mod fs;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[cfg(feature = "full")]
    #[error("Quic connection error: {0}")]
    Quic(#[from] ConnectionError),

    #[error("Config directory not found")]
    Config,

    #[error("Serde error: {0}")]
    SerDe(#[from] serde_bencode::Error),

    #[cfg(feature = "full")]
    #[error("Cipher error: {0}")]
    Cipher(#[from] crate::crypto::Error),

    #[error("Unexpected message received from network")]
    UnexpectedMessage,

    #[error("{0}")]
    Custom(&'static str),
}

impl<'a> From<message::Error<'a>> for Error {
    fn from(value: message::Error<'a>) -> Self {
        match value.0 {
            #[cfg(feature = "full")]
            message::ErrorKind::Decryption(error) => Self::Cipher(error),
            message::ErrorKind::CipherExpected
            | message::ErrorKind::InsuficientBuffer
            | message::ErrorKind::InvalidMessage => Self::UnexpectedMessage,
        }
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::Custom(value)
    }
}
