use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::AeadInPlace;
use aes_gcm_siv::{Aes256GcmSiv, KeyInit};
use rand::RngCore;
use thiserror::Error;

#[derive(Clone)]
pub struct Cipher {
    key: Vec<u8>,
    aead: Aes256GcmSiv,
    associated_data: Option<Vec<u8>>,
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
    pub fn new(key: Vec<u8>) -> Self {
        let aead = Aes256GcmSiv::new(key.as_slice().into());

        Self {
            aead,
            associated_data: None,
            key,
        }
    }

    pub fn get_key(&self) -> &Vec<u8> {
        &self.key
    }

    pub fn set_associated_data(&mut self, associated_data: Vec<u8>) {
        self.associated_data = Some(associated_data);
    }

    #[must_use]
    pub fn encrypt(&mut self, plain: &mut [u8]) -> ([u8; 12], [u8; 16]) {
        let mut nonce = GenericArray::default();

        rand::thread_rng().fill_bytes(&mut nonce);

        let associated_data = match self.associated_data {
            Some(ref ad) => ad,
            None => &self.key,
        };

        let tag = self
            .aead
            .encrypt_in_place_detached(&nonce, associated_data, plain)
            .expect("Associated data an plan are not bigger than expected in aes_gcm");

        (nonce.into(), tag.into())
    }

    pub fn decrypt(
        &mut self,
        cipher: &mut [u8],
        nonce: &[u8; 12],
        tag: &[u8; 16],
    ) -> Result<(), DecryptError> {
        let associated_data = match self.associated_data {
            Some(ref ad) => ad,
            None => &self.key,
        };
        self.aead
            .decrypt_in_place_detached(nonce.into(), associated_data, cipher, tag.into())
            .map_err(|_| DecryptError)
    }
}

#[derive(Debug, Error)]
#[error("Failure verifying MAC")]
pub struct DecryptError;
