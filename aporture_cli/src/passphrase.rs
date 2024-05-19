use anyhow::{bail, Result};
use colored::Colorize;

use aporture::fs::contacts::Contacts;

pub enum Method<'a> {
    Direct(String),
    Generate,
    Contact(&'a str, &'a mut Contacts),
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
            println!("Share it with your {}", "peer".bright_cyan());

            Ok(passphrase.into_bytes())
        }
        Method::Contact(name, contacts) => match contacts.get(name) {
            Some(key) => Ok(key.to_vec()),
            None => bail!("Contact {name} not found"),
        },
    }
}
