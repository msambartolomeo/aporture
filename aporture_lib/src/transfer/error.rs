use thiserror::Error;

#[derive(Debug, Error)]
pub enum Send {
    #[error("The selected file is not a regular file, it may be a folder")]
    Path,
    #[error("Could not open file to send")]
    File(std::io::Error),
    #[error("Could not send file to peer over the network")]
    Network(crate::io::Error),
    #[error("Hash mismatch informed by the receiver")]
    HashMismatch,
}

impl From<std::io::Error> for Send {
    fn from(value: std::io::Error) -> Self {
        Self::File(value)
    }
}

impl From<crate::io::Error> for Send {
    fn from(value: crate::io::Error) -> Self {
        Self::Network(value)
    }
}

#[derive(Debug, Error)]
pub enum Receive {
    #[error("Could not write file to disk")]
    File(std::io::Error),
    #[error("Could not received file to peer over the network")]
    Network(crate::io::Error),
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

impl From<crate::io::Error> for Receive {
    fn from(value: crate::io::Error) -> Self {
        match value {
            crate::io::Error::IO(_) | crate::io::Error::SerDe(_) => Self::Network(value),
            crate::io::Error::Cipher(e) => Self::Cipher(e),
            crate::io::Error::Custom(_) => unreachable!(),
            crate::io::Error::Config => unreachable!(),
        }
    }
}
