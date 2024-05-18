use super::Key;

#[derive(Default)]
pub struct Hasher {
    hasher: blake3::Hasher,
}

pub type Hash = [u8; 32];
pub type Salt = [u8; 16];

impl Hasher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            hasher: blake3::Hasher::default(),
        }
    }

    pub fn add(&mut self, input: &[u8]) {
        self.hasher.update(input);
    }

    #[must_use]
    pub fn finalize(self) -> Hash {
        self.hasher.finalize().into()
    }

    #[must_use]
    pub fn hash(input: &[u8]) -> Hash {
        blake3::hash(input).into()
    }

    #[must_use]
    pub fn derive_key(password: &[u8], salt: &[u8]) -> Key {
        let mut key = Hash::default();

        let hasher = argon2::Argon2::default();

        hasher
            .hash_password_into(password, salt, &mut key)
            .expect("Valid out length");

        key
    }
}
