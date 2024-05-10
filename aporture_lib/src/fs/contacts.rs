use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Contacts {
    key_map: HashMap<String, Vec<u8>>,
}

impl Contacts {
    pub async fn load() -> Self {
        todo!()
    }

    pub async fn save(self) {
        todo!()
    }

    pub fn get_key(&self, contact: String) -> Vec<u8> {
        todo!()
    }

    pub fn store_key(&mut self, contact: String, key: Vec<u8>) {
        todo!()
    }
}
