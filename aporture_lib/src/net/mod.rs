use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::crypto::Cipher;
use crate::protocol::Parser;

mod error;
pub use error::Error;

#[derive(Debug)]
pub struct NetworkPeer {
    cipher: Option<Cipher>,
    stream: TcpStream,
}

impl NetworkPeer {
    pub const fn new(stream: TcpStream) -> Self {
        Self {
            cipher: None,
            stream,
        }
    }

    pub fn add_cipher(&mut self, cipher: Cipher) {
        self.cipher = Some(cipher);
    }

    pub fn extract_cipher(&mut self) -> Option<Cipher> {
        self.cipher.take()
    }

    pub async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error> {
        let in_buf = input.serialize_to();

        self.stream.write_all(&in_buf.len().to_be_bytes()).await?;

        self.stream.write_all(&in_buf).await?;

        Ok(())
    }

    pub async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, Error> {
        let mut length = [0; 8];

        self.stream.read_exact(&mut length).await?;

        let length = usize::from_be_bytes(length);

        if length == P::serialized_size() {
            let mut buffer = P::buffer();

            self.stream.read_exact(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        } else {
            let mut buffer = vec![0; length];

            self.stream.read_exact(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        }
    }

    pub async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error> {
        if self.cipher.is_none() {
            return Err(Error::NoCipher);
        }

        let mut buf = input.serialize_to();

        self.stream.write_all(&buf.len().to_be_bytes()).await?;

        self.write_enc(&mut buf).await?;

        Ok(())
    }

    pub async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), Error> {
        let Some(cipher) = &mut self.cipher else {
            return Err(Error::NoCipher);
        };

        let (nonce, tag) = cipher.encrypt(input);

        self.stream.write_all(&nonce).await?;
        self.stream.write_all(input).await?;
        self.stream.write_all(&tag).await?;

        Ok(())
    }

    pub async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, Error> {
        if self.cipher.is_none() {
            return Err(Error::NoCipher);
        }

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

    pub async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        let Some(cipher) = &mut self.cipher else {
            return Err(Error::NoCipher);
        };

        let mut nonce = [0; 12];
        let mut tag = [0; 16];

        self.stream.read_exact(&mut nonce).await?;
        self.stream.read_exact(buffer).await?;
        self.stream.read_exact(&mut tag).await?;

        cipher.decrypt(buffer, &nonce, &tag)?;

        Ok(())
    }
}
