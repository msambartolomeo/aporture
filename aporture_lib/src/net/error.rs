use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Network failure: {0}")]
    IO(std::io::Error),

    #[error("Serde error: {0}")]
    SerDe(serde_bencode::Error),

    #[error("Cipher error: {0}")]
    Cipher(crate::crypto::Error),

    #[error("No cipher configured for this method.")]
    NoCipher,
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

impl From<crate::crypto::Error> for Error {
    fn from(value: crate::crypto::Error) -> Self {
        Self::Cipher(value)
    }
}
