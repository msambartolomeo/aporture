use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};

use crate::crypto::cipher::Cipher;
use crate::crypto::hasher::Hasher;
use crate::crypto::Key;
use crate::fs::config::Config;
use crate::fs::EncryptedFileManager;
use crate::parse;
use crate::parser::{EncryptedSerdeIO, Parser};

const CONTACTS_FILE_NAME: &str = "contacts.app";

#[derive(Debug)]
pub struct Contacts {
    content: Content,
    manager: EncryptedFileManager,
}

#[derive(Debug, Serialize, Deserialize)]
struct Contact {
    pub key: [u8; 32],
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Content {
    map: HashMap<String, Contact>,
}

parse!(Content);

impl Contacts {
    #[must_use]
    pub fn exists() -> bool {
        path().map(|p| p.exists()).unwrap_or(false)
    }

    pub async fn empty(password: &[u8]) -> Result<Self, crate::io::Error> {
        let path = path()?;

        let key = Hasher::derive_key(password, &Config::get().await.password_salt);

        let cipher = Cipher::new(&key);

        let manager = EncryptedFileManager::new(path, cipher);

        Ok(Self {
            content: Content::default(),
            manager,
        })
    }

    pub async fn load(password: &[u8]) -> Result<Self, crate::io::Error> {
        let path = path()?;

        let key = Hasher::derive_key(password, &Config::get().await.password_salt);

        let cipher = Cipher::new(&key);

        let mut manager = EncryptedFileManager::new(path, cipher);

        log::info!("Reading contacts from {}", manager);

        let contacts = manager.read_ser_enc().await?;

        Ok(Self {
            content: contacts,
            manager,
        })
    }

    pub async fn save(&mut self) -> Result<(), crate::io::Error> {
        log::info!("Saving contacts to {}", self.manager);

        self.manager.write_ser_enc(&self.content).await.ok();

        Ok(())
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&[u8; 32]> {
        self.content.map.get(name).map(|c| &c.key)
    }

    pub fn add(&mut self, name: String, key: Key) {
        let timestamp = chrono::Local::now();

        let contact = Contact { key, timestamp };

        self.content.map.insert(name, contact);
    }

    pub fn replace(&mut self, new_name: String, old_name: Option<String>, key: Key) {
        if let Some(name) = old_name {
            self.delete(&name);
        }

        self.add(new_name, key);
    }

    pub fn delete(&mut self, name: &str) -> bool {
        self.content.map.remove(name).is_some()
    }

    pub fn list(&self) -> impl Iterator<Item = (&String, DateTime<Local>)> {
        self.content.map.iter().map(|(n, c)| (n, c.timestamp))
    }
}

fn path() -> Result<PathBuf, crate::io::Error> {
    let mut path = crate::fs::path()?;

    path.push(CONTACTS_FILE_NAME);

    Ok(path)
}
