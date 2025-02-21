use anyhow::{Result, anyhow, bail};
use tokio::sync::OnceCell;

use aporture::fs::contacts::Contacts;

#[derive(Default)]
pub struct Holder(OnceCell<Contacts>);

impl Holder {
    pub async fn get_or_init(&self) -> Result<&Contacts> {
        self.0
            .get_or_try_init(|| async {
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

                    contacts.ok_or_else(|| anyhow!("3 incorrect password attempts"))
                } else {
                    println!("No contacts registered, creating database...");
                    let mut password = None;

                    for _ in 0..3 {
                        let p1 =
                            rpassword::prompt_password("Enter password to encrypt contacts: ")?;
                        let p2 =
                            rpassword::prompt_password("Reenter password to encrypt contacts: ")?;

                        if p1 == p2 {
                            password = Some(p1);
                            break;
                        }

                        println!("Passwords do not match, try again");
                    }

                    let password =
                        password.ok_or_else(|| anyhow!("3 incorrect password attempts"))?;

                    let contacts = Contacts::empty(&password.into_bytes()).await?;

                    Ok(contacts)
                }
            })
            .await
    }

    pub async fn get_mut_or_init(&mut self) -> Result<&mut Contacts> {
        let _ = self.get_or_init().await?;

        Ok(self.0.get_mut().expect("Initialized in get_or_init()"))
    }

    pub async fn save(mut self) -> Result<()> {
        if let Some(contacts) = self.0.get_mut() {
            contacts.save().await?;
        }
        Ok(())
    }
}
