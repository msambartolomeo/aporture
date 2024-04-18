use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::crypto::{Cipher, DecryptError};
use crate::protocol::Parser;

#[derive(Debug)]
pub struct NetworkPeer {
    cipher: Option<Cipher>,
    stream: TcpStream,
}

impl NetworkPeer {
    pub fn new(stream: TcpStream) -> Self {
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

    pub async fn write_ser<P: Parser>(&mut self, input: &P) -> Result<(), Error> {
        let in_buf = input.serialize_to();

        self.stream
            .write_all(&in_buf.len().to_be_bytes())
            .await
            .map_err(Error::IO)?;

        self.stream.write_all(&in_buf).await.map_err(Error::IO)
    }

    pub async fn read_ser<P: Parser>(&mut self) -> Result<P, Error> {
        let mut length = [0; 8];

        self.stream
            .read_exact(&mut length)
            .await
            .map_err(Error::IO)?;

        let length = usize::from_be_bytes(length);

        if length == P::serialized_size() {
            let mut buffer = P::buffer();

            self.stream
                .read_exact(&mut buffer)
                .await
                .map_err(Error::IO)?;

            P::deserialize_from(&buffer).map_err(Error::SerDe)
        } else {
            let mut buffer = vec![0; length];

            self.stream
                .read_exact(&mut buffer)
                .await
                .map_err(Error::IO)?;

            P::deserialize_from(&buffer).map_err(Error::SerDe)
        }
    }

    pub async fn write_ser_enc<P: Parser>(&mut self, input: &P) -> Result<(), Error> {
        if self.cipher.is_none() {
            return Err(Error::NoCipher);
        }

        let mut buf = input.serialize_to();

        self.stream
            .write_all(&buf.len().to_be_bytes())
            .await
            .map_err(Error::IO)?;

        self.write_enc(&mut buf).await
    }

    pub async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), Error> {
        let Some(cipher) = &mut self.cipher else {
            return Err(Error::NoCipher);
        };

        let (nonce, tag) = cipher.encrypt(input);

        self.stream.write_all(&nonce).await.map_err(Error::IO)?;
        self.stream.write_all(&input).await.map_err(Error::IO)?;
        self.stream.write_all(&tag).await.map_err(Error::IO)?;

        Ok(())
    }

    pub async fn read_ser_enc<P: Parser>(&mut self) -> Result<P, Error> {
        if self.cipher.is_none() {
            return Err(Error::NoCipher);
        }

        let mut length = [0; 8];

        self.stream
            .read_exact(&mut length)
            .await
            .map_err(Error::IO)?;

        let length = usize::from_be_bytes(length);

        if length == P::serialized_size() {
            let mut buffer = P::buffer();

            self.read_enc(&mut buffer).await?;

            P::deserialize_from(&buffer).map_err(Error::SerDe)
        } else {
            let mut buffer = vec![0; length];

            self.read_enc(&mut buffer).await?;

            P::deserialize_from(&buffer).map_err(Error::SerDe)
        }
    }

    pub async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        let Some(cipher) = &mut self.cipher else {
            return Err(Error::NoCipher);
        };

        let mut nonce = [0; 12];
        let mut tag = [0; 16];

        self.stream
            .read_exact(&mut nonce)
            .await
            .map_err(Error::IO)?;
        self.stream.read_exact(buffer).await.map_err(Error::IO)?;
        self.stream.read_exact(&mut tag).await.map_err(Error::IO)?;

        cipher.decrypt(buffer, &nonce, &tag).map_err(Error::Cipher)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Network failure: {0}")]
    IO(std::io::Error),

    #[error("Serde error: {0}")]
    SerDe(serde_bencode::Error),

    #[error("Cipher error: {0}")]
    Cipher(DecryptError),

    #[error("No cipher configured for this method.")]
    NoCipher,
}
