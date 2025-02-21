use anyhow::{Result, bail};
use colored::Colorize;

use aporture::fs::contacts::Contacts;

#[derive(Debug)]
pub enum Method<'a> {
    Direct(String),
    Generate,
    Contact(&'a str, &'a Contacts),
}

pub fn get(method: Method) -> Result<Vec<u8>> {
    match method {
        Method::Direct(passphrase) => Ok(passphrase.into_bytes()),
        Method::Generate => {
            let passphrase = aporture::passphrase::generate(3);

            println!(
                "The generated passphrase is '{}'",
                passphrase.green().bold()
            );
            println!(
                "Share it with your {}",
                "peer".bright_cyan().bold().underline()
            );

            Ok(passphrase.into_bytes())
        }
        Method::Contact(name, contacts) => match contacts.get(name) {
            Some(key) => {
                println!(
                    "Using key associated with contact {}",
                    name.bright_blue().bold()
                );
                Ok(key.to_vec())
            }
            None => bail!("Contact {name} not found"),
        },
    }
}
