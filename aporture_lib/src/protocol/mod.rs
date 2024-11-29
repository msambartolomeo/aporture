use std::net::SocketAddr;

use generic_array::typenum as n;
use generic_array::{typenum::Unsigned, GenericArray};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes, DisplayFromStr};

#[macro_use]
pub mod parser;
use parser::Parser;

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Receiver = 1,
}
parse!(PairKind, size: n::U3);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Hello {
    /// Protocol version
    pub version: u8,

    /// Pair Kind
    pub kind: PairKind,

    #[serde_as(as = "Bytes")]
    pub pair_id: [u8; 32],
}
parse!(Hello, size: n::U67);

impl Hello {
    #[must_use]
    pub const fn new(kind: PairKind, pair_id: [u8; 32]) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            kind,
            pair_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairingResponseCode {
    // NOTE: Okay types
    Ok = 0,
    OkSamePublicIP = 3,

    // NOTE: Error types
    UnsupportedVersion = 1,
    NoPeer = 4,
    MalformedMessage = 5,
}
parse!(PairingResponseCode, size: n::U3);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyExchangePayload(#[serde_as(as = "Bytes")] pub [u8; 33]);
parse!(KeyExchangePayload, size: n::U36);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct NegotiationPayload {
    pub addresses: Vec<SocketAddr>,
    #[serde_as(as = "DisplayFromStr")]
    pub save_contact: bool,
}
parse!(NegotiationPayload);

#[serde_as]
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferData {
    pub total_files: u64,

    pub total_size: u64,

    pub root_name: String,

    #[serde_as(as = "DisplayFromStr")]
    pub compressed: bool,
}
parse!(TransferData);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileData {
    pub file_size: u64,

    pub file_name: String,

    #[serde_as(as = "DisplayFromStr")]
    pub is_file: bool,
}
parse!(FileData);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum TransferResponseCode {
    Ok = 0,
    HashMismatch = 1,
}
parse!(TransferResponseCode, size: n::U3);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Hash(#[serde_as(as = "Bytes")] pub [u8; 32]);
parse!(Hash, size: n::U35);

// UDP HOLE PUNCHING

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum HolePunchingRequest {
    Address = 0,
    Relay = 1,
    None = 2,
}
parse!(HolePunchingRequest, size: n::U3);

parse!(SocketAddr, size: n::U24);

#[cfg(test)]
mod test {
    use super::*;

    use paste::paste;

    macro_rules! test_parsed {
        ($type:ident, $example:expr) => {
            paste! {
                #[test]
                #[allow(non_snake_case)]
                fn [<test_ $type _parser>]() -> Result<(), Box<dyn std::error::Error>> {
                    let input: $type = $example;

                    let serialized = input.serialize_to();

                    if let Some(len) = $type::serialized_size() {
                        assert!(len >= serialized.len());
                    }

                    let deserialized = $type::deserialize_from(&serialized)?;

                    assert_eq!(input, deserialized);

                    Ok(())
                }
            }
        };
    }

    test_parsed!(PairingResponseCode, PairingResponseCode::Ok);

    test_parsed!(
        Hello,
        Hello {
            version: 0,
            kind: PairKind::Sender,
            pair_id: Default::default(),
        }
    );

    test_parsed!(PairKind, PairKind::Sender);

    test_parsed!(KeyExchangePayload, KeyExchangePayload([0; 33]));

    test_parsed!(
        NegotiationPayload,
        NegotiationPayload {
            addresses: vec![SocketAddr::from(([0, 0, 0, 0], 0))],
            save_contact: true,
        }
    );

    test_parsed!(
        TransferData,
        TransferData {
            total_files: 1,
            total_size: 2,
            root_name: "/hello".to_owned(),
            compressed: false,
        }
    );

    test_parsed!(
        FileData,
        FileData {
            file_size: 1,
            is_file: false,
            file_name: "pepe".to_owned(),
        }
    );

    test_parsed!(TransferResponseCode, TransferResponseCode::Ok);

    test_parsed!(Hash, Hash([0; 32]));

    test_parsed!(SocketAddr, ([200, 200, 200, 200], 65535).into());

    test_parsed!(HolePunchingRequest, HolePunchingRequest::Address);
}
