use thiserror::Error;

#[derive(Debug, Error)]
pub enum Send {
    #[error("The selected file is not a regular file, it may be a folder")]
    Path,
    #[error("Could not open file to send")]
    File(std::io::Error),
    #[error("Could not send file to peer over the network")]
    Network(crate::net::Error),
    #[error("Hash mismatch informed by the receiver")]
    HashMismatch,
}

impl From<std::io::Error> for Send {
    fn from(value: std::io::Error) -> Self {
        Self::File(value)
    }
}

impl From<crate::net::Error> for Send {
    fn from(value: crate::net::Error) -> Self {
        Self::Network(value)
    }
}

#[derive(Debug, Error)]
pub enum Receive {
    #[error("Could not write file to disk")]
    File(std::io::Error),
    #[error("Could not received file to peer over the network")]
    Network(crate::net::Error),
    #[error("Error in cryptography: {0}")]
    Cipher(crate::crypto::Error),
    #[error("The hash of the transefered file and the received hash are not the same")]
    HashMismatch,
}

impl From<std::io::Error> for Receive {
    fn from(value: std::io::Error) -> Self {
        Self::File(value)
    }
}

impl From<crate::net::Error> for Receive {
    fn from(value: crate::net::Error) -> Self {
        match value {
            crate::net::Error::IO(_) | crate::net::Error::SerDe(_) => Self::Network(value),
            crate::net::Error::Cipher(e) => Self::Cipher(e),
            crate::net::Error::NoCipher => unreachable!(),
        }
    }
}
