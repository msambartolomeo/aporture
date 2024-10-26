use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use generic_array::GenericArray;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::crypto::hasher::Salt;
use crate::parse;
use crate::parser::{Parser, SerdeIO};

use crate::fs::FileManager;

const CONFIG_FILE_NAME: &str = "config.app";

const DEFAULT_SERVER_ADDRESS: Option<&str> = option_env!("SERVER_ADDRESS");
const DEFAULT_SERVER_PORT: u16 = 8765;

static CONFIG: OnceCell<Config> = OnceCell::const_new();

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server_address: IpAddr,
    pub server_port: u16,
    pub password_salt: Salt,
}

parse!(Config);

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

impl Config {
    pub async fn get() -> &'static Self {
        if let Some(config) = CONFIG.get() {
            config
        } else {
            CONFIG
                .get_or_init(|| async {
                    if let Ok(config) = Self::from_file().await {
                        config
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

    pub fn server_address(&self) -> SocketAddr {
        (self.server_address, self.server_port).into()
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

        config_dir.push(CONFIG_FILE_NAME);

        log::info!("Getting config from {}", config_dir.display());

        let config = Self::default();

        let mut manager = FileManager::new(config_dir);

        manager.write_ser(&config).await.ok();

        Ok(config)
    }

    fn path() -> Result<PathBuf, crate::io::Error> {
        let mut path = crate::fs::path()?;

        path.push(CONFIG_FILE_NAME);

        Ok(path)
    }
}
