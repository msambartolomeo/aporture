use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use tokio::sync::{OnceCell, RwLock, RwLockReadGuard};

use crate::parse;
use crate::parser::{Parser, SerdeIO};

use crate::fs::FileManager;

const CONFIG_FILE_NAME: &str = "config.app";

const DEFAULT_SERVER_ADDRESS: Option<&str> = option_env!("SERVER_ADDRESS");
const DEFAULT_SERVER_PORT: u16 = 8765;

static CONFIG: OnceCell<RwLock<Config>> = OnceCell::const_new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Config {
    server_domain: String,
    server_address: IpAddr,
    server_port: u16,
}

parse!(Config);

impl Config {
    async fn default() -> Self {
        let server_domain = DEFAULT_SERVER_ADDRESS
            .unwrap_or("aporture.duckdns.org")
            .to_string();

        let address = lookup_host(&server_domain)
            .await
            .unwrap_or_else(|_| ([127, 0, 0, 1], DEFAULT_SERVER_PORT).into());

        Self {
            server_domain,
            server_address: address.ip(),
            server_port: address.port(),
        }
    }

    pub async fn get() -> RwLockReadGuard<'static, Self> {
        if let Some(config) = CONFIG.get() {
            config.read().await
        } else {
            CONFIG
                .get_or_init(|| async {
                    let config = if let Ok(config) = Self::from_file().await {
                        config
                    } else {
                        log::info!("Using default config");
                        log::warn!("Could not find config file, creating");
                        if let Ok(c) = Self::create_file().await {
                            c
                        } else {
                            log::warn!("Error creating config file");
                            Self::default().await
                        }
                    };

                    RwLock::new(config)
                })
                .await
                .read()
                .await
        }
    }

    #[must_use]
    pub fn server_address(&self) -> SocketAddr {
        (self.server_address, self.server_port).into()
    }

    #[must_use]
    pub fn server_domain(&self) -> &str {
        &self.server_domain
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

        let config = Self::default().await;

        let mut manager = FileManager::new(config_dir);

        manager.write_ser(&config).await.ok();

        Ok(config)
    }

    pub async fn update_address(
        address: String,
    ) -> Result<RwLockReadGuard<'static, Self>, crate::io::Error> {
        if !CONFIG.initialized() {
            let _ = Self::get().await;
        }

        let mut config = CONFIG.get().expect("Should be created above").write().await;

        let server_address = lookup_host(&address).await?;

        config.server_domain = address;
        config.server_address = server_address.ip();
        config.server_port = server_address.port();

        config.save().await?;

        Ok(config.downgrade())
    }

    async fn save(&self) -> Result<(), crate::io::Error> {
        let path = Self::path()?;

        log::info!("Saving config to {}", path.display());

        let mut manager = FileManager::new(path);

        manager.write_ser(self).await?;

        Ok(())
    }

    fn path() -> Result<PathBuf, crate::io::Error> {
        let mut path = crate::fs::path()?;

        path.push(CONFIG_FILE_NAME);

        Ok(path)
    }
}

async fn lookup_host(address: &str) -> Result<SocketAddr, crate::io::Error> {
    if let Ok(a) = tokio::net::lookup_host(address.to_owned()).await {
        a
    } else {
        tokio::net::lookup_host(format!("{address}:{DEFAULT_SERVER_PORT}")).await?
    }
    .find(std::net::SocketAddr::is_ipv4)
    .ok_or(crate::io::Error::Config)
}
