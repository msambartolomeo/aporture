use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::crypto::Cipher;
use crate::fs::EncryptedFileManager;
use crate::parser::{EncryptedSerdeIO, Parser};

#[derive(Debug, Serialize, Deserialize)]
pub struct Contacts {
    map: HashMap<String, Contact>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub key: Vec<u8>,
    pub timestamp: DateTime<Local>,
}

impl Parser for Contacts {
    type MinimumSerializedSize = generic_array::typenum::U0;
}

impl Contacts {
    fn path() -> PathBuf {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")
            .expect("PC must have valid home directory");
        let mut config_dir = dirs.config_dir().to_path_buf();
        config_dir.push("contacts");

        config_dir
    }

    pub async fn load(cipher: Arc<Cipher>) -> Result<Self, crate::io::Error> {
        let path = Contacts::path();

        let mut manager = EncryptedFileManager::new(&path, cipher);

        let config = manager.read_ser_enc().await?;

        Ok(config)
    }

    pub fn exists() -> bool {
        Contacts::path().exists()
    }

    pub async fn save(self, cipher: Arc<Cipher>) -> Result<(), crate::io::Error> {
        let path = Contacts::path();

        tokio::fs::create_dir_all(&path).await?;

        let mut manager = EncryptedFileManager::new(&path, cipher);

        manager.write_ser_enc(&self).await.ok();

        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Contact> {
        self.map.get(name)
    }

    pub fn add(&mut self, name: String, key: Vec<u8>) {
        let timestamp = chrono::Local::now();

        let contact = Contact { key, timestamp };

        self.map.insert(name, contact);
    }

    pub fn delete(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, &Contact)> {
        self.map.iter()
    }
}
