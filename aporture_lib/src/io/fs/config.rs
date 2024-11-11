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

static mut CONFIG: OnceCell<Config> = OnceCell::const_new();

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
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
        // SAFETY: Safe by SAFETY guarantees from exclusive access functions
        if let Some(config) = unsafe { CONFIG.get() } {
            config
        } else {
            // SAFETY: Safe because exclusive access takes self, so they must have called this function first
            unsafe {
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
    }

    #[must_use]
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

    ///
    /// # Safety
    /// This function is safe to call in an async context
    /// as mutable access does not happen between await points.
    /// It is unsafe in threaded context as it can be interrupted.
    ///
    pub async unsafe fn persist_server_address_change(self) -> Result<Self, crate::io::Error> {
        // SAFETY: Refer to function safety
        let mut config = unsafe { CONFIG.take() }
            .expect("Should have been constructed to get a self to pass to this function");

        config.server_address = self.server_address;
        config.server_port = self.server_port;

        // SAFETY: Refer to function safety
        unsafe { CONFIG.set(config) }.expect("Should be empty as take has been called");

        config.save().await?;

        Ok(config)
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
