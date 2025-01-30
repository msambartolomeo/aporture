use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::peer::{Encryptable, Peer};
use super::plain::NetworkPeer;
use crate::crypto::cipher::Cipher;

pub struct EncryptedNetworkPeer {
    cipher: Arc<Cipher>,
    peer: NetworkPeer,
}

impl NetworkPeer {
    #[must_use]
    pub const fn add_cipher(self, cipher: Arc<Cipher>) -> EncryptedNetworkPeer {
        EncryptedNetworkPeer { cipher, peer: self }
    }
}

impl EncryptedNetworkPeer {
    #[must_use]
    pub const fn new(stream: TcpStream, cipher: Arc<Cipher>) -> Self {
        let peer = NetworkPeer::new(stream);

        Self { cipher, peer }
    }

    #[must_use]
    pub fn extract_cipher(self) -> (NetworkPeer, Arc<Cipher>) {
        (self.peer, self.cipher)
    }
}

impl Peer for EncryptedNetworkPeer {
    fn writer(&mut self) -> impl AsyncWriteExt + Unpin {
        self.peer.writer()
    }

    fn reader(&mut self) -> impl AsyncReadExt + Unpin {
        self.peer.reader()
    }
}

impl Encryptable for EncryptedNetworkPeer {
    fn cipher(&self) -> impl AsRef<Cipher> {
        &self.cipher
    }
}
