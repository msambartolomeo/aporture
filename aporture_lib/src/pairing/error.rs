use thiserror::Error;

use crate::protocol::PROTOCOL_VERSION;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Hello(#[from] Hello),
    #[error(transparent)]
    KeyExchange(#[from] KeyExchange),
    #[error(transparent)]
    AddressExchange(#[from] Negotiation),
}

#[derive(Debug, Error)]
pub enum Hello {
    #[error("Could not connect to server: {0}")]
    NoServer(#[from] std::io::Error),
    #[error("Peer has not yet arrived")]
    NoPeer,
    #[error("The selected server does not implement APP version {PROTOCOL_VERSION}")]
    ServerUnsupportedVersion,
    #[error("Server behaved incorrectly on connection: {0}")]
    ServerError(#[from] crate::io::Error),
    #[error("Message send to server was invalid")]
    ClientError,
}

#[derive(Debug, Error)]
pub enum KeyExchange {
    #[error("Error exchanging key with peer: {0}")]
    NetworkError(#[from] crate::io::Error),
    #[error("Invalid key derivation")]
    KeyDerivationError,
}

impl From<spake2::Error> for KeyExchange {
    fn from(_: spake2::Error) -> Self {
        Self::KeyDerivationError
    }
}

#[derive(Debug, Error)]
#[error("Error exchanging defined addresses with peer: {0}")]
pub struct Negotiation(#[from] crate::io::Error);
