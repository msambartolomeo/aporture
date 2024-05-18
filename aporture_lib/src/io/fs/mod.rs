use std::{path::Path, sync::Arc};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::crypto::cipher::Cipher;
use crate::parser::{EncryptedSerdeIO, Parser, SerdeIO};

pub mod config;
pub mod contacts;

struct FileManager<'a> {
    path: &'a Path,
}

impl<'a> FileManager<'a> {
    pub const fn new(path: &'a Path) -> Self {
        FileManager { path }
    }
}

impl<'a> SerdeIO for FileManager<'a> {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let buffer = input.serialize_to();

        tokio::fs::write(self.path, buffer).await?;

        Ok(())
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        let buffer = tokio::fs::read(self.path).await?;

        let deserialized = P::deserialize_from(&buffer)?;

        Ok(deserialized)
    }
}

struct EncryptedFileManager<'a> {
    manager: FileManager<'a>,
    cipher: Arc<Cipher>,
}

impl<'a> EncryptedFileManager<'a> {
    pub fn new(path: &'a Path, cipher: Arc<Cipher>) -> Self {
        let manager = FileManager::new(path);
        EncryptedFileManager { manager, cipher }
    }
}

impl<'a> SerdeIO for EncryptedFileManager<'a> {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        self.manager.write_ser(input).await
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        self.manager.read_ser().await
    }
}

impl<'a> EncryptedSerdeIO for EncryptedFileManager<'a> {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let mut buffer = input.serialize_to();

        self.write_enc(&mut buffer).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error> {
        let (nonce, tag) = self.cipher.encrypt(input);

        let mut file = tokio::fs::File::create(self.manager.path).await?;
        file.write_all(&nonce).await?;
        file.write_all(input).await?;
        file.write_all(&tag).await?;

        Ok(())
    }

    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        let len = tokio::fs::metadata(self.manager.path).await?.len();
        let len = usize::try_from(len).expect("File size is bigger than system usize") - 12 - 16;

        let mut buffer = vec![0; len];

        self.read_enc(&mut buffer).await?;

        let deserialized = P::deserialize_from(&buffer)?;

        Ok(deserialized)
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), crate::io::Error> {
        let mut nonce = [0; 12];
        let mut tag = [0; 16];

        let mut file = tokio::fs::File::open(self.manager.path).await?;

        file.read_exact(&mut nonce).await?;
        file.read_exact(buffer).await?;
        file.read_exact(&mut tag).await?;

        self.cipher.decrypt(buffer, &nonce, &tag)?;

        Ok(())
    }
}
