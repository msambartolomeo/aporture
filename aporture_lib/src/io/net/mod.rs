#[cfg(feature = "full")]
pub mod cipher;
pub mod plain;

#[cfg(feature = "full")]
pub use cipher::EncryptedNetworkPeer;
pub use plain::NetworkPeer;
