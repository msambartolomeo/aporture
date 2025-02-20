use std::path::PathBuf;

use generic_array::GenericArray;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::crypto::hasher;
use crate::parse;
use crate::parser::{Parser, SerdeIO};

use crate::fs::FileManager;

const SALT_FILE_NAME: &str = "salt.app";

static SALT: OnceCell<Salt> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub(crate) struct Salt(pub hasher::Salt);
parse!(Salt);

impl Default for Salt {
    fn default() -> Self {
        let mut salt = hasher::Salt::default();

        rand::rng().fill_bytes(&mut salt);

        Self(salt)
    }
}

impl Salt {
    pub async fn get() -> &'static Self {
        if let Some(salt) = SALT.get() {
            salt
        } else {
            SALT.get_or_init(|| async {
                if let Ok(salt) = Self::from_file().await {
                    salt
                } else {
                    log::info!("Using default config");
                    log::warn!("Could not find config file, creating");
                    Self::create_file()
                        .await
                        .inspect_err(|_| log::warn!("Error creating config file"))
                        .unwrap_or_default()
                }
            })
            .await
        }
    }

    async fn from_file() -> Result<Self, crate::io::Error> {
        let path = Self::path()?;

        log::info!("Getting config from {}", path.display());

        let mut manager = FileManager::new(path);

        let config = manager.read_ser().await?;

        Ok(config)
    }

    async fn create_file() -> Result<Self, crate::io::Error> {
        let mut config_dir = crate::fs::path()?;

        tokio::fs::create_dir_all(&config_dir).await?;

        config_dir.push(SALT_FILE_NAME);

        log::info!("Getting config from {}", config_dir.display());

        let config = Self::default();

        let mut manager = FileManager::new(config_dir);

        manager.write_ser(&config).await.ok();

        Ok(config)
    }

    fn path() -> Result<PathBuf, crate::io::Error> {
        let mut path = crate::fs::path()?;

        path.push(SALT_FILE_NAME);

        Ok(path)
    }
}
