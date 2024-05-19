pub mod cipher;
mod error;
pub mod hasher;
pub use error::Error;

pub type Key = [u8; 32];
