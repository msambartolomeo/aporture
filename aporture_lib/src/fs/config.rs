use std::sync::OnceLock;

const DEFAULT_SERVER_ADDRESS: Option<&str> = option_env!("SERVER_ADDRESS");
const DEFAULT_SERVER_PORT: u16 = 8765;

static CONFIG: OnceLock<Config> = OnceLock::new();

pub struct Config {
    pub server_address: String,
    pub server_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_address: DEFAULT_SERVER_ADDRESS.unwrap_or("127.0.0.1").to_owned(),
            server_port: DEFAULT_SERVER_PORT,
        }
    }
}

impl<'a> Config {
    pub async fn get() -> &'a Self {
        match CONFIG.get() {
            Some(config) => config,
            None => {
                let config = Config::from_file().await.unwrap_or_default();

                CONFIG.get_or_init(|| config)
            }
        }
    }

    async fn from_file() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("dev", "msambartolomeo", "aporture")?;

        let config_dir = dirs.config_dir();

        tokio::fs::create_dir_all(config_dir).await.ok()?;

        todo!()
    }
}
