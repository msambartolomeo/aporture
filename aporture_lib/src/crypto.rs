use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::Aead;
use aes_gcm_siv::{Aes256GcmSiv, KeyInit, Nonce};
use rand::rngs::ThreadRng;
use rand::RngCore;

pub type Key = Vec<u8>;

pub struct Crypto {
    key: Vec<u8>,
    aead: Aes256GcmSiv,
    rng: ThreadRng,
}

impl Crypto {
    #[must_use]
    pub fn new(key: Vec<u8>) -> Self {
        let aead = Aes256GcmSiv::new(key.as_slice().into());

        let rng = rand::thread_rng();

        Self { key, aead, rng }
    }

    #[must_use]
    pub fn encrypt(&mut self, plain: &[u8]) -> (Vec<u8>, Nonce) {
        let mut nonce = GenericArray::default();

        self.rng.fill_bytes(&mut nonce);

        let cipher = self.aead.encrypt(&nonce, plain).expect("Encryption works");

        (cipher, nonce)
    }

    #[must_use]
    pub fn decrypt(&mut self, cipher: &[u8], nonce: Nonce) -> Vec<u8> {
        self.aead.decrypt(&nonce, cipher).expect("Decryption works")
    }

    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }
}
