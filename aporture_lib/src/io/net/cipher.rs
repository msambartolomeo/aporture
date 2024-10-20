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

        let message = Message::new(&mut serialized);

        let mut buf = message.into_buf();

        self.stream().write_all_buf(&mut buf).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error> {
        let message = Message::new_encrypted(input, self.cipher.as_ref());

        self.stream().write_all_buf(&mut message.into_buf()).await?;

        Ok(())
    }

    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new_encrypted(&mut buffer, &self.cipher);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream().read_buf(&mut buf).await?;
            }

            let n = buf.consume_encrypted(&self.cipher)?;

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new_encrypted(&mut buffer, self.cipher.as_ref());

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream().read_buf(&mut buf).await?;
            }

            let n = buf.consume_encrypted(&self.cipher)?;

            Ok(P::deserialize_from(&buffer[..n])?)
        }
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<usize, crate::io::Error> {
        let message = Message::new_encrypted(buffer, &self.cipher);

        let mut buf = message.into_buf();

        while buf.has_remaining_mut() {
            self.stream().read_buf(&mut buf).await?;
        }

        let n = buf.consume_encrypted(&self.cipher)?;

        Ok(n)
    }
}
