use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{crypto::Cipher, protocol::Parser};

use super::{Error, NetworkPeer, SerdeNetwork};

pub struct EncryptedNetworkPeer {
    cipher: Arc<Cipher>,
    peer: NetworkPeer,
}

#[allow(async_fn_in_trait)]
pub trait EncryptedSerdeNetwork: SerdeNetwork {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error>;
    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), Error>;
    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, Error>;
    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), Error>;
}

impl EncryptedNetworkPeer {
    pub fn new(stream: TcpStream, cipher: Arc<Cipher>) -> Self {
        let peer = NetworkPeer::new(stream);

        Self { cipher, peer }
    }

    pub fn extract_cipher(self) -> (NetworkPeer, Arc<Cipher>) {
        (self.peer, self.cipher)
    }
}

impl Deref for EncryptedNetworkPeer {
    type Target = NetworkPeer;

    fn deref(&self) -> &Self::Target {
        &self.peer
    }
}

impl DerefMut for EncryptedNetworkPeer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.peer
    }
}

impl SerdeNetwork for EncryptedNetworkPeer {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error> {
        self.peer.write_ser(input).await
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, Error> {
        self.peer.read_ser().await
    }
}

impl EncryptedSerdeNetwork for EncryptedNetworkPeer {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error> {
        let mut buf = input.serialize_to();

        self.peer.stream.write_all(&buf.len().to_be_bytes()).await?;

        self.write_enc(&mut buf).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), Error> {
        let (nonce, tag) = self.cipher.encrypt(input);

        self.stream.write_all(&nonce).await?;
        self.stream.write_all(input).await?;
        self.stream.write_all(&tag).await?;

        Ok(())
    }

    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, Error> {
        let mut length = [0; 8];

        self.stream.read_exact(&mut length).await?;

        let length = usize::from_be_bytes(length);

        if length == P::serialized_size() {
            let mut buffer = P::buffer();

            self.read_enc(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        } else {
            let mut buffer = vec![0; length];

            self.read_enc(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        }
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        let mut nonce = [0; 12];
        let mut tag = [0; 16];

        self.stream.read_exact(&mut nonce).await?;
        self.stream.read_exact(buffer).await?;
        self.stream.read_exact(&mut tag).await?;

        self.cipher.decrypt(buffer, &nonce, &tag)?;

        Ok(())
    }
}
