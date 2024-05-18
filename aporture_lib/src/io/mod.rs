use thiserror::Error;

pub mod net;
pub mod parser;

#[cfg(feature = "full")]
pub mod fs;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(std::io::Error),

    #[error("Config directory not found")]
    Config,

    #[error("Serde error: {0}")]
    SerDe(serde_bencode::Error),

    #[cfg(feature = "full")]
    #[error("Cipher error: {0}")]
    Cipher(crate::crypto::Error),

    #[error("{0}")]
    Custom(&'static str),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<serde_bencode::Error> for Error {
    fn from(value: serde_bencode::Error) -> Self {
        Self::SerDe(value)
    }
}

#[cfg(feature = "full")]
impl From<crate::crypto::Error> for Error {
    fn from(value: crate::crypto::Error) -> Self {
        Self::Cipher(value)
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::Custom(value)
    }
}
