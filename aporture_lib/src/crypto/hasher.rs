use super::Key;

pub struct Hasher {
    hasher: blake3::Hasher,
}

pub type Hash = [u8; 32];
pub type Salt = [u8; 16];

impl Hasher {
    pub fn new() -> Self {
        Hasher {
            hasher: blake3::Hasher::new(),
        }
    }

    pub fn add(&mut self, input: &[u8]) {
        self.hasher.update(input);
    }

    pub fn finalize(self) -> Hash {
        self.hasher.finalize().into()
    }

    pub fn hash(input: &[u8]) -> Hash {
        blake3::hash(input).into()
    }

    pub fn derive_key(password: &[u8], salt: &[u8]) -> Key {
        let mut key = Hash::default();

        let hasher = argon2::Argon2::default();

        hasher
            .hash_password_into(password, &salt, &mut key)
            .expect("Valid out length");

        key
    }
}
