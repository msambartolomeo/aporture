use std::net::IpAddr;
use std::sync::OnceLock;

use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::crypto::hasher::Salt;
use crate::parser::{Parser, SerdeIO};

use super::FileManager;

const DEFAULT_SERVER_ADDRESS: Option<&str> = option_env!("SERVER_ADDRESS");
const DEFAULT_SERVER_PORT: u16 = 8765;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server_address: IpAddr,
    pub server_port: u16,
    pub password_salt: Salt,
}

impl Parser for Config {
    type MinimumSerializedSize = generic_array::typenum::U0;
}

impl Default for Config {
    fn default() -> Self {
        let mut salt = Salt::default();

        rand::thread_rng().fill_bytes(&mut salt);

        Self {
            server_address: DEFAULT_SERVER_ADDRESS
                .unwrap_or("127.0.0.1")
                .parse()
                .unwrap_or_else(|_| IpAddr::from([127, 0, 0, 1])),
            server_port: DEFAULT_SERVER_PORT,
            password_salt: salt,
        }
    }
}

impl<'a> Config {
    pub async fn get() -> &'a Self {
        if let Some(config) = CONFIG.get() {
            config
        } else {
            let config = if let Some(config) = Self::from_file().await {
                config
            } else {
                log::info!("Using default config");
                log::warn!("Could not find config file, creating");
                Self::create_file().await.unwrap_or_else(|| {
                    log::warn!("Error creating config file");
                    Self::default()
                })
            };

            CONFIG.get_or_init(|| config)
        }
    }

    async fn from_file() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")?;
        let mut config_dir = dirs.config_dir().to_path_buf();
        config_dir.push("config");

        let mut manager = FileManager::new(&config_dir);

        let config = manager.read_ser().await.ok()?;

        Some(config)
    }

    async fn create_file() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")?;
        let mut config_dir = dirs.config_dir().to_path_buf();

        tokio::fs::create_dir_all(&config_dir).await.ok()?;

        config_dir.push("config");

        let config = Self::default();

        let mut manager = FileManager::new(&config_dir);

        manager.write_ser(&config).await.ok();

        Some(config)
    }
}
