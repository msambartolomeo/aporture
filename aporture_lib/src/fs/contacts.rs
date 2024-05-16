use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::crypto::Cipher;
use crate::parser::{EncryptedSerdeIO, Parser};

use super::EncryptedFileManager;

#[derive(Serialize, Deserialize)]
struct Contacts {
    key_map: HashMap<String, Vec<u8>>,
}

impl Parser for Contacts {
    type MinimumSerializedSize = generic_array::typenum::U0;
}

impl Contacts {
    pub async fn load(cipher: Arc<Cipher>) -> Result<Self, crate::parser::Error> {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")
            .ok_or("Cannot find valid project directory")?;
        let mut config_dir = dirs.config_dir().to_path_buf();
        config_dir.push("contacts");

        let mut manager = EncryptedFileManager::new(&config_dir, cipher);

        let config = manager.read_ser_enc().await?;

        Ok(config)
    }

    pub async fn save(self, cipher: Arc<Cipher>) -> Result<(), crate::parser::Error> {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")
            .ok_or("Cannot find valid project directory")?;
        let mut config_dir = dirs.config_dir().to_path_buf();

        tokio::fs::create_dir_all(&config_dir).await?;

        config_dir.push("config");

        let mut manager = EncryptedFileManager::new(&config_dir, cipher);

        manager.write_ser_enc(&self).await.ok();

        Ok(())
    }

    pub fn get_key(&self, contact: &str) -> Option<&Vec<u8>> {
        self.key_map.get(contact)
    }

    pub fn store_key(&mut self, contact: String, key: Vec<u8>) {
        self.key_map.insert(contact, key);
    }
}
