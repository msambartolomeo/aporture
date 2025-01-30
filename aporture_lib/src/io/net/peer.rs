use bytes::BufMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::net::message::Message;
use crate::parser::SerdeIO;

#[cfg(feature = "full")]
use crate::crypto::cipher::Cipher;
#[cfg(feature = "full")]
use crate::parser::EncryptedSerdeIO;

pub trait Peer {
    fn writer(&mut self) -> impl AsyncWriteExt + Unpin + Send;
    fn reader(&mut self) -> impl AsyncReadExt + Unpin + Send;
}

#[cfg(feature = "full")]
pub trait Encryptable {
    fn cipher(&self) -> impl AsRef<Cipher>;
}

impl<T: Peer + Send> SerdeIO for T {
    async fn write_ser<P: crate::parser::Parser + Sync>(
        &mut self,
        input: &P,
    ) -> Result<(), crate::io::Error> {
        let mut serialized = input.serialize_to();

        let message = Message::new(&mut serialized);

        let mut buf = message.into_buf();

        self.writer().write_all_buf(&mut buf).await?;

        Ok(())
    }

    async fn read_ser<P: crate::parser::Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new(&mut buffer);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.reader().read_buf(&mut buf).await?;
            }

            let n = buf.consume()?;

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new(&mut buffer);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.reader().read_buf(&mut buf).await?;
            }

            let n = buf.consume()?;

            Ok(P::deserialize_from(&buffer[..n])?)
        }
    }
}

#[cfg(feature = "full")]
impl<T: Peer + Encryptable + Send> EncryptedSerdeIO for T {
    async fn write_ser_enc<P: crate::parser::Parser + Sync>(
        &mut self,
        input: &P,
    ) -> Result<(), crate::io::Error> {
        let mut serialized = input.serialize_to();

        let message = Message::new_encrypted(&mut serialized, self.cipher().as_ref());

        let mut buf = message.into_buf();

        self.writer().write_all_buf(&mut buf).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error> {
        let message = Message::new_encrypted(input, self.cipher().as_ref());

        self.writer().write_all_buf(&mut message.into_buf()).await?;

        Ok(())
    }

    async fn read_ser_enc<P: crate::parser::Parser + Sync>(
        &mut self,
    ) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new_encrypted(&mut buffer, self.cipher().as_ref());

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.reader().read_buf(&mut buf).await?;
            }

            let n = buf.consume_encrypted(self.cipher().as_ref())?;

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new_encrypted(&mut buffer, self.cipher().as_ref());

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.reader().read_buf(&mut buf).await?;
            }

            let n = buf.consume_encrypted(self.cipher().as_ref())?;

            Ok(P::deserialize_from(&buffer[..n])?)
        }
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<usize, crate::io::Error> {
        let message = Message::new_encrypted(buffer, self.cipher().as_ref());

        let mut buf = message.into_buf();

        while buf.has_remaining_mut() {
            self.reader().read_buf(&mut buf).await?;
        }

        let n = buf.consume_encrypted(self.cipher().as_ref())?;

        Ok(n)
    }
}
