pub mod cipher;
mod error;
pub mod hasher;
pub use error::Error;

pub type Key = [u8; 32];

// #[derive(Debug)]
// pub enum Key {
//     Direct([u8; 32]),
//     Generated { key: [u8; 32], salt: [u8; 16] },
// }

// impl TryFrom<Vec<u8>> for Key {
//     type Error = <[u8; 32] as TryFrom<Vec<u8>>>::Error;

//     fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
//         Ok(Self::Direct(value.try_into()?))
//     }
// }

// impl Key {
//     pub fn bytes(&self) -> &[u8; 32] {
//         match self {
//             Key::Direct(key) => key,
//             Key::Generated { key, .. } => key,
//         }
//     }

//     pub fn into_bytes(self) -> [u8; 32] {
//         match self {
//             Key::Direct(key) => key,
//             Key::Generated { key, .. } => key,
//         }
//     }

//     pub fn salt(&self) -> Option<&[u8; 16]> {
//         match self {
//             Key::Direct(_) => None,
//             Key::Generated { salt, .. } => Some(salt),
//         }
//     }
// }
