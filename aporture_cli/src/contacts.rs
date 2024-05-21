use anyhow::{anyhow, bail, Result};

use aporture::fs::contacts::Contacts;

#[derive(Default)]
pub struct Holder(Option<Contacts>);

impl Holder {
    pub async fn get_or_init(&mut self) -> Result<&mut Contacts> {
        if self.0.is_none() {
            if Contacts::exists() {
                let mut contacts = None;

                for _ in 0..3 {
                    let password =
                        rpassword::prompt_password("Insert contact database password: ")?;

                    match Contacts::load(&password.into_bytes()).await {
                        Ok(c) => {
                            contacts = Some(c);
                            break;
                        }
                        Err(aporture::io::Error::Cipher(_)) => {
                            println!("Sorry, try again");
                            continue;
                        }
                        Err(_) => bail!("Could not find or create contacts file"),
                    };
                }

                let contacts = contacts.ok_or_else(|| anyhow!("3 incorrect password attempts"))?;

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

                let password = password.ok_or_else(|| anyhow!("3 incorrect password attempts"))?;

                let contacts = Contacts::empty(&password.into_bytes()).await?;

                self.0 = Some(contacts);
            }
        }

        let Some(contacts) = self.0.as_mut() else {
            unreachable!("Already initialized before")
        };
        Ok(contacts)
    }

    pub async fn save(self) -> Result<()> {
        if let Some(mut contacts) = self.0 {
            contacts.save().await?;
        }
        Ok(())
    }
}
