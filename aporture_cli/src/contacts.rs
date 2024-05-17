use std::sync::Arc;

use anyhow::{Context, Ok, Result};

use aporture::crypto::Cipher;
use aporture::fs::contacts::Contacts;

#[derive(Default)]
pub struct ContactsHolder(Option<(Arc<Cipher>, Contacts)>);

impl ContactsHolder {
    pub async fn get_or_init(&mut self) -> Result<&mut Contacts> {
        if self.0.is_none() {
            if !Contacts::exists() {
                println!("No contacts registered, creating database...");
                let passphrase = rpassword::prompt_password("Insert password to encrypt contacts")?;
                let cipher = Arc::new(Cipher::new(passphrase.into_bytes()));
                self.0 = Some((cipher, Contacts::default()));
            } else {
                let passphrase = rpassword::prompt_password("Insert password to read contacts")?;
                let cipher = Arc::new(Cipher::new(passphrase.into_bytes()));
                let contacts = Contacts::load(cipher.clone())
                    .await
                    .context("Could not find or create contacts file")?;
                self.0 = Some((cipher.clone(), contacts));
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
