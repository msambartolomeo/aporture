use std::sync::Arc;

use anyhow::{bail, Result};

use aporture::crypto::Cipher;
use aporture::fs::contacts::Contacts;

#[derive(Default)]
pub struct Holder(Option<(Arc<Cipher>, Contacts)>);

impl Holder {
    pub async fn get_or_init(&mut self) -> Result<&mut Contacts> {
        if self.0.is_none() {
            if Contacts::exists() {
                loop {
                    let password = rpassword::prompt_password("Insert password to read contacts")?;
                    let cipher = Arc::new(Cipher::new(password.into_bytes()));
                    let contacts = match Contacts::load(cipher.clone()).await {
                        Ok(contacts) => contacts,
                        Err(aporture::io::Error::Cipher(_)) => {
                            println!("Incorrect password, retrying");
                            continue;
                        }
                        Err(_) => bail!("Could not find or create contacts file"),
                    };

                    self.0 = Some((cipher.clone(), contacts));

                    break;
                }
            } else {
                println!("No contacts registered, creating database...");
                let password = loop {
                    let p1 = rpassword::prompt_password("Enter password to encrypt contacts:")?;
                    let p2 = rpassword::prompt_password("Reenter password to encrypt contacts:")?;

                    if p1 != p2 {
                        println!("Password does not match, retrying..");
                        continue;
                    }
                    break p1;
                };

                let cipher = Arc::new(Cipher::new(password.into_bytes()));
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
