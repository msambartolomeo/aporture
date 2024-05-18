use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::crypto::cipher::Cipher;
use crate::fs::EncryptedFileManager;
use crate::parser::{EncryptedSerdeIO, Parser};

const CONTACTS_FILE_NAME: &str = "contacts.app";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Contacts {
    map: HashMap<String, Contact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Contact {
    pub key: [u8; 32],
    pub timestamp: DateTime<Local>,
}

impl Parser for Contacts {
    type MinimumSerializedSize = generic_array::typenum::U0;
}

impl Contacts {
    fn path() -> Result<PathBuf, crate::io::Error> {
        let mut path = crate::fs::path()?;

        path.push(CONTACTS_FILE_NAME);

        Ok(path)
    }

    #[must_use]
    pub fn exists() -> bool {
        Self::path().map(|p| p.exists()).unwrap_or(false)
    }

    pub async fn load(cipher: Arc<Cipher>) -> Result<Self, crate::io::Error> {
        let path = Self::path()?;

        let mut manager = EncryptedFileManager::new(&path, cipher);

        let config = manager.read_ser_enc().await?;

        Ok(config)
    }

    pub async fn save(self, cipher: Arc<Cipher>) -> Result<(), crate::io::Error> {
        let path = Self::path()?;

        log::info!("Saving contacts to {}", path.display());

        let mut manager = EncryptedFileManager::new(&path, cipher);

        manager.write_ser_enc(&self).await.ok();

        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&[u8; 32]> {
        self.map.get(name).map(|c| &c.key)
    }

    pub fn add(&mut self, name: String, key: [u8; 32]) {
        let timestamp = chrono::Local::now();

        let contact = Contact { key, timestamp };

        self.map.insert(name, contact);
    }

    pub fn delete(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, DateTime<Local>)> {
        self.map.iter().map(|(n, c)| (n, c.timestamp))
    }
}
