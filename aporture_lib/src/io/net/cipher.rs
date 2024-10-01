use std::sync::Arc;

use bytes::BufMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::message::Message;
use super::plain::NetworkPeer;
use crate::crypto::cipher::Cipher;
use crate::parser::{EncryptedSerdeIO, Parser, SerdeIO};

pub struct EncryptedNetworkPeer {
    cipher: Arc<Cipher>,
    peer: NetworkPeer,
}

impl NetworkPeer {
    pub const fn add_cipher(self, cipher: Arc<Cipher>) -> EncryptedNetworkPeer {
        EncryptedNetworkPeer { cipher, peer: self }
    }
}

impl EncryptedNetworkPeer {
    pub const fn new(stream: TcpStream, cipher: Arc<Cipher>) -> Self {
        let peer = NetworkPeer::new(stream);

        Self { cipher, peer }
    }

    pub fn extract_cipher(self) -> (NetworkPeer, Arc<Cipher>) {
        (self.peer, self.cipher)
    }

    fn stream(&mut self) -> &mut TcpStream {
        self.peer.inner()
    }
}

impl SerdeIO for EncryptedNetworkPeer {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        self.peer.write_ser(input).await
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        self.peer.read_ser().await
    }
}

impl EncryptedSerdeIO for EncryptedNetworkPeer {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let mut serialized = input.serialize_to();

        let message = Message::new(&mut serialized, None);

        let mut buf = message.into_buf();

        self.stream().write_all_buf(&mut buf).await?;

        // let mut buf = input.serialize_to();

        // self.stream().write_all(&buf.len().to_be_bytes()).await?;

        // self.write_enc(&mut buf).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error> {
        let message = Message::new(input, Some(self.cipher.as_ref()));

        self.stream().write_all_buf(&mut message.into_buf()).await?;

        // let (nonce, tag) = self.cipher.encrypt(input);

        // self.stream().write_all(&nonce).await?;
        // self.stream().write_all(input).await?;
        // self.stream().write_all(&tag).await?;

        Ok(())
    }

    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new(&mut buffer, Some(self.cipher.as_ref()));

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream().read_buf(&mut buf).await?;
            }

            let n = buf.consume(None)?;

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new(&mut buffer, Some(self.cipher.as_ref()));

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream().read_buf(&mut buf).await?;
            }

            let n = buf.consume(None)?;

            Ok(P::deserialize_from(&buffer[..n])?)
        }

        // let mut length = [0; 8];

        // self.stream().read_exact(&mut length).await?;

        // let length = usize::from_be_bytes(length);

        // if length == P::serialized_size() {
        //     let mut buffer = P::buffer();

        //     self.read_enc(&mut buffer).await?;

        //     let deserialized = P::deserialize_from(&buffer)?;

        //     Ok(deserialized)
        // } else {
        //     let mut buffer = vec![0; length];

        //     self.read_enc(&mut buffer).await?;

        //     let deserialized = P::deserialize_from(&buffer)?;

        //     Ok(deserialized)
        // }
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<usize, crate::io::Error> {
        let message = Message::new(buffer, Some(self.cipher.as_ref()));

        let mut buf = message.into_buf();

        while buf.has_remaining_mut() {
            self.stream().read_buf(&mut buf).await?;
        }

        let n = buf.consume(Some(self.cipher.as_ref()))?;

        // let mut nonce = [0; 12];
        // let mut tag = [0; 16];

        // self.stream().read_exact(&mut nonce).await?;
        // self.stream().read_exact(buffer).await?;
        // self.stream().read_exact(&mut tag).await?;

        // self.cipher.decrypt(buffer, &nonce, &tag)?;

        Ok(n)
    }
}
