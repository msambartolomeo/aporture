use std::sync::Arc;

use anyhow::{anyhow, bail, Result};

use aporture::crypto::cipher::Cipher;
use aporture::crypto::hasher::Hasher;
use aporture::fs::config::Config;
use aporture::fs::contacts::Contacts;

#[derive(Default)]
pub struct Holder(Option<(Arc<Cipher>, Contacts)>);

impl Holder {
    pub async fn get_or_init(&mut self) -> Result<&mut Contacts> {
        let config = Config::get().await;

        if self.0.is_none() {
            if Contacts::exists() {
                let mut contacts = None;

                for _ in 0..3 {
                    let password =
                        rpassword::prompt_password("Insert contact database password: ")?;

                    let key = Hasher::derive_key(&password.into_bytes(), &config.password_salt);

                    let cipher = Arc::new(Cipher::new(&key));
                    match Contacts::load(cipher.clone()).await {
                        Ok(c) => {
                            contacts = Some((cipher, c));
                            break;
                        }
                        Err(aporture::io::Error::Cipher(_)) => {
                            println!("Sorry, try again");
                            continue;
                        }
                        Err(_) => bail!("Could not find or create contacts file"),
                    };
                }

                let contacts = contacts.ok_or(anyhow!("3 incorrect password attempts"))?;

                self.0 = Some(contacts);
            } else {
                println!("No contacts registered, creating database...");
                let mut password = None;

                for _ in 0..3 {
                    let p1 = rpassword::prompt_password("Enter password to encrypt contacts: ")?;
                    let p2 = rpassword::prompt_password("Reenter password to encrypt contacts: ")?;

                    if p1 == p2 {
                        password = Some(p1);
                        break;
                    }

                    println!("Passwords do not match, try again");
                }

                let password = password.ok_or(anyhow!("3 incorrect password attempts"))?;

                let key = Hasher::derive_key(&password.into_bytes(), &config.password_salt);

                let cipher = Arc::new(Cipher::new(&key));
                self.0 = Some((cipher, Contacts::default()));
            }
        }

        match self.0.as_mut() {
            Some((_, c)) => Ok(c),
            None => unreachable!("Already initialized before"),
        }
    }

    pub async fn save(self) -> Result<()> {
        if let Some((cipher, contacts)) = self.0 {
            contacts.save(cipher).await?;
        }
        Ok(())
    }
}
