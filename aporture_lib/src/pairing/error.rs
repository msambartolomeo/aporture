use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Hello(Hello),
    #[error("{0}")]
    KeyExchange(KeyExchange),
    #[error("{0}")]
    AddressExchange(AddressExchange),
}

impl From<Hello> for Error {
    fn from(value: Hello) -> Self {
        Self::Hello(value)
    }
}

impl From<KeyExchange> for Error {
    fn from(value: KeyExchange) -> Self {
        Self::KeyExchange(value)
    }
}

impl From<AddressExchange> for Error {
    fn from(value: AddressExchange) -> Self {
        Self::AddressExchange(value)
    }
}

#[derive(Debug, Error)]
pub enum Hello {
    #[error("Could not connect to server: {0}")]
    NoServer(std::io::Error),
    #[error("Peer has not yet arrived")]
    NoPeer,
    #[error(
        "The selected server does not implement APP version {}",
        super::PROTOCOL_VERSION
    )]
    ServerUnsupportedVersion,
    #[error("Server behaved incorrectly on connection: {0}")]
    ServerError(crate::net::Error),
    #[error("Message send to server was invalid")]
    ClientError,
}

impl From<crate::net::Error> for Hello {
    fn from(value: crate::net::Error) -> Self {
        Self::ServerError(value)
    }
}

impl From<std::io::Error> for Hello {
    fn from(value: std::io::Error) -> Self {
        Self::NoServer(value)
    }
}

#[derive(Debug, Error)]
pub enum KeyExchange {
    #[error("Error exchanging key with peer: {0}")]
    NetworkError(crate::net::Error),
    #[error("Invalid key derivation")]
    KeyDerivationError,
}

impl From<crate::net::Error> for KeyExchange {
    fn from(value: crate::net::Error) -> Self {
        Self::NetworkError(value)
    }
}

impl From<spake2::Error> for KeyExchange {
    fn from(_: spake2::Error) -> Self {
        Self::KeyDerivationError
    }
}

#[derive(Debug, Error)]
#[error("Error exchanging defined addresses with peer: {0}")]
pub struct AddressExchange(crate::net::Error);

impl From<crate::net::Error> for AddressExchange {
    fn from(value: crate::net::Error) -> Self {
        Self(value)
    }
}
