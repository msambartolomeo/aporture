use aes_gcm_siv::aead::AeadInPlace;
use aes_gcm_siv::{Aes256GcmSiv, KeyInit};
use rand::RngCore;

pub use super::Error;
use super::Key;

pub type Nonce = [u8; 12];
pub type Tag = [u8; 16];

#[derive(Clone)]
pub struct Cipher {
    aead: Aes256GcmSiv,
    associated_data: Vec<u8>,
}

impl std::fmt::Debug for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cipher")
            .field("aead", &"Hidden implementation")
            .field("associated_data", &self.associated_data)
            .finish()
    }
}

impl Cipher {
    #[must_use]
    pub fn new(key: &Key) -> Self {
        let aead = Aes256GcmSiv::new(key.into());

        Self {
            aead,
            associated_data: b"".to_vec(),
        }
    }

    pub fn set_associated_data(&mut self, associated_data: Vec<u8>) {
        self.associated_data = associated_data;
    }

    #[must_use]
    pub fn encrypt(&self, plain: &mut [u8]) -> ([u8; 12], [u8; 16]) {
        let mut nonce = Nonce::default();
        rand::rng().fill_bytes(&mut nonce);

        let tag = self
            .aead
            .encrypt_in_place_detached(&nonce.into(), &self.associated_data, plain)
            .expect("Associated data an plan are not bigger than expected in aes_gcm");

        (nonce, tag.into())
    }

    pub fn decrypt(&self, cipher: &mut [u8], nonce: &Nonce, tag: &Tag) -> Result<(), Error> {
        self.aead.decrypt_in_place_detached(
            nonce.into(),
            &self.associated_data,
            cipher,
            tag.into(),
        )?;

        Ok(())
    }
}
