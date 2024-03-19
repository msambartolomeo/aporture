use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};

pub trait BencodeSerDe: Serialize + for<'a> Deserialize<'a> {
    const SERIALIZED_SIZE: usize;

    fn serialize(&self) -> Vec<u8> {
        serde_bencode::to_bytes(self)
            .inspect_err(|e| log::error!("Unkown error when serializing type {e}"))
            .expect("Serialization should not fail because the type is valid")
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, serde_bencode::Error> {
        serde_bencode::from_bytes(buffer)
    }
}

#[derive(Debug, Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Reciever = 1,
}

impl BencodeSerDe for PairKind {
    const SERIALIZED_SIZE: usize = 3;
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct APPHello {
    /// Protocol version
    pub version: u8,

    /// Pair Kind
    pub kind: PairKind,

    #[serde_as(as = "Bytes")]
    pub pair_id: [u8; 64],
}

impl BencodeSerDe for APPHello {
    const SERIALIZED_SIZE: usize = 99;
}

#[derive(Debug, Clone, Copy, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ResponseCode {
    // NOTE: Okay types
    Ok = 0,
    OkSamePublicIP = 3,

    // NOTE: Error types
    UnsupportedVersion = 1,
    NoPeer = 4,
    MalformedMessage = 5,
}

impl BencodeSerDe for ResponseCode {
    const SERIALIZED_SIZE: usize = 3;
}
