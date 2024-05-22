use std::fmt::Display;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::crypto::cipher::Cipher;
use crate::parser::{EncryptedSerdeIO, Parser, SerdeIO};

pub mod config;
pub mod contacts;

#[derive(Debug)]
struct FileManager {
    path: PathBuf,
}

impl FileManager {
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SerdeIO for FileManager {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let buffer = input.serialize_to();

        tokio::fs::write(&self.path, buffer).await?;

        Ok(())
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        let buffer = tokio::fs::read(&self.path).await?;

        let deserialized = P::deserialize_from(&buffer)?;

        Ok(deserialized)
    }
}

#[derive(Debug)]
struct EncryptedFileManager {
    manager: FileManager,
    cipher: Cipher,
}

impl EncryptedFileManager {
    pub fn new(path: PathBuf, cipher: Cipher) -> Self {
        let manager = FileManager::new(path);
        Self { manager, cipher }
    }
}

impl SerdeIO for EncryptedFileManager {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        self.manager.write_ser(input).await
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        self.manager.read_ser().await
    }
}

impl EncryptedSerdeIO for EncryptedFileManager {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let mut buffer = input.serialize_to();

        self.write_enc(&mut buffer).await?;

        Ok(())
    }

    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error> {
        let (nonce, tag) = self.cipher.encrypt(input);

        let mut file = tokio::fs::File::create(&self.manager.path).await?;
        file.write_all(&nonce).await?;
        file.write_all(input).await?;
        file.write_all(&tag).await?;

        Ok(())
    }

    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        let len = tokio::fs::metadata(&self.manager.path).await?.len();

        let len = usize::try_from(len).expect("File size is bigger than system usize") - 12 - 16;

        let mut buffer = vec![0; len];

        self.read_enc(&mut buffer).await?;

        let deserialized = P::deserialize_from(&buffer)?;

        Ok(deserialized)
    }

    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), crate::io::Error> {
        let mut nonce = [0; 12];
        let mut tag = [0; 16];

        let mut file = tokio::fs::File::open(&self.manager.path).await?;

        file.read_exact(&mut nonce).await?;
        file.read_exact(buffer).await?;
        file.read_exact(&mut tag).await?;

        self.cipher.decrypt(buffer, &nonce, &tag)?;

        Ok(())
    }
}

fn path() -> Result<PathBuf, crate::io::Error> {
    let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")
        .ok_or(crate::io::Error::Config)?;

    Ok(dirs.config_dir().to_path_buf())
}

impl Display for FileManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

impl Display for EncryptedFileManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.manager)
    }
}

#[must_use]
pub fn downloads_directory() -> Option<PathBuf> {
    directories::UserDirs::new().and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
}
