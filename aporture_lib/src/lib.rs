pub mod io;
pub mod protocol;

pub use io::net;
pub use protocol::parser;

#[cfg(feature = "full")]
pub use io::fs;

#[cfg(feature = "full")]
pub mod crypto;
#[cfg(feature = "full")]
pub mod pairing;
#[cfg(feature = "full")]
pub mod passphrase;
#[cfg(feature = "full")]
pub mod transfer;

pub trait State {}
pub struct Sender {}
impl State for Sender {}
pub struct Receiver {}
impl State for Receiver {}
