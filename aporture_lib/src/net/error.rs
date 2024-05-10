use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Network failure: {0}")]
    IO(std::io::Error),

    #[error("Serde error: {0}")]
    SerDe(serde_bencode::Error),

    #[cfg(feature = "full")]
    #[error("Cipher error: {0}")]
    Cipher(crate::crypto::Error),

    #[cfg(feature = "full")]
    #[error("External protocol error")]
    Protocol,
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
