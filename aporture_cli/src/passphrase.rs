use anyhow::{bail, Result};
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

            println!("The generated passphrase is '{passphrase}'");
            println!("Share it with your peer");

            Ok(passphrase.into_bytes())
        }
        Method::Contact(name, contacts) => match contacts.get(name) {
            Some(key) => Ok(key.clone()),
            None => bail!("Contact {name} not found"),
        },
    }
}
