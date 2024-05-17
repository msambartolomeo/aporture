use anyhow::{bail, Result};
use aporture::fs::contacts::Contacts;

pub enum PassphraseMethod<'a> {
    Direct(String),
    Generate,
    Contact(&'a str, &'a mut Contacts),
}

pub fn get_passphrase(method: PassphraseMethod) -> Result<Vec<u8>> {
    match method {
        PassphraseMethod::Direct(passphrase) => Ok(passphrase.into_bytes()),
        PassphraseMethod::Generate => {
            let passphrase = aporture::passphrase::generate(3);

            println!("The generated passphrase is {passphrase}");
            println!("Share it with your peer");

            Ok(passphrase.into_bytes())
        }
        PassphraseMethod::Contact(name, contacts) => match contacts.get(&name) {
            Some(key) => Ok(key.clone()),
            None => bail!("Contact {name} not found"),
        },
    }
}
