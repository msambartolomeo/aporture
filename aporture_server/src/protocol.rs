use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};

#[derive(Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Reciever = 1,
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct AporturePairingProtocol {
    /// Protocol version
    pub version: u8,

    /// Pair Kind
    pub kind: PairKind,

    #[serde_as(as = "Bytes")]
    pub pair_id: [u8; 64],
}

impl AporturePairingProtocol {
    // TODO: Check if there is a way to calculate it automatically
    pub const fn serialized_size() -> usize {
        99
    }
}

#[derive(Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ResponseCode {
    // NOTE: Okay types
    Ok = 0,
    OkSamePublicIP = 3,

    // NOTE: Error types
    UnsupportedVersion = 1,
    NoSender = 4,
}
