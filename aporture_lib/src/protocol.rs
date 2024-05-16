use std::ffi::OsString;
use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};

use crate::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Receiver = 1,
}

pub const PROTOCOL_VERSION: u8 = 1;
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

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyExchangePayload(#[serde_as(as = "Bytes")] pub [u8; 33]);

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyConfirmationPayload {
    #[serde_as(as = "Bytes")]
    // NOTE: Must be aporture
    pub tag: [u8; 8],

    // NOTE: milis from epoch as bytes
    // #[serde_as(as = "Bytes")]
    pub timestamp: Duration,
}

impl Default for KeyConfirmationPayload {
    fn default() -> Self {
        Self {
            tag: b"aporture".to_owned(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("Now is after unix epoch"),
        }
    }
}

#[serde_as]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileData {
    #[serde_as(as = "Bytes")]
    pub hash: [u8; 32],

    // NOTE: Size of file in network byte order
    #[serde_as(as = "Bytes")]
    pub file_size: [u8; 8],

    pub file_name: OsString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum TransferResponseCode {
    Ok = 0,
    HashMismatch = 1,
}

impl Parser for PairKind {
    type MinimumSerializedSize = generic_array::typenum::U3;
}

impl Parser for Hello {
    type MinimumSerializedSize = generic_array::typenum::U67;
}

impl Parser for PairingResponseCode {
    type MinimumSerializedSize = generic_array::typenum::U3;
}

impl Parser for KeyExchangePayload {
    type MinimumSerializedSize = generic_array::typenum::U36;
}

impl Parser for KeyConfirmationPayload {
    type MinimumSerializedSize = generic_array::typenum::U66;
}

impl Parser for SocketAddr {
    type MinimumSerializedSize = generic_array::typenum::U11;
}

impl Parser for FileData {
    type MinimumSerializedSize = generic_array::typenum::U85;
}

impl Parser for TransferResponseCode {
    type MinimumSerializedSize = generic_array::typenum::U3;
}

impl<P: Parser> Parser for Vec<P> {
    type MinimumSerializedSize = P::MinimumSerializedSize;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_response_ser_de() {
        let response = PairingResponseCode::Ok;

        let serialized = response.serialize_to();

        assert_eq!(PairingResponseCode::serialized_size(), serialized.len());

        let deserialized = PairingResponseCode::deserialize_from(&serialized).unwrap();

        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_app_hello_ser_de() {
        let hello = Hello {
            version: 0,
            kind: PairKind::Sender,
            pair_id: Default::default(),
        };

        let serialized = hello.serialize_to();

        assert_eq!(Hello::serialized_size(), serialized.len());

        let deserialized = Hello::deserialize_from(&serialized).unwrap();

        assert_eq!(hello, deserialized);
    }

    #[test]
    fn test_pair_kind_ser_de() {
        let pair = PairKind::Sender;

        let serialized = pair.serialize_to();

        assert_eq!(PairKind::serialized_size(), serialized.len());

        let deserialized = PairKind::deserialize_from(&serialized).unwrap();

        assert_eq!(pair, deserialized);
    }

    #[test]
    fn test_key_exchange_ser_de() {
        let key_exchange = KeyExchangePayload([0; 33]);

        let serialized = key_exchange.serialize_to();

        assert_eq!(KeyExchangePayload::serialized_size(), serialized.len());

        let deserialized = KeyExchangePayload::deserialize_from(&serialized).unwrap();

        assert_eq!(key_exchange, deserialized);
    }

    #[test]
    fn test_address_ser_de() {
        let address = SocketAddr::from(([0, 0, 0, 0], 0));

        let serialized = address.serialize_to();

        assert_eq!(SocketAddr::serialized_size(), serialized.len());

        let deserialized = SocketAddr::deserialize_from(&serialized).unwrap();

        assert_eq!(address, deserialized);
    }

    #[test]
    fn test_file_data_ser_de() {
        let file_data = FileData {
            hash: [1; 32],
            file_size: 1usize.to_be_bytes(),
            file_name: OsString::new(),
        };

        let serialized = file_data.serialize_to();

        assert_eq!(FileData::serialized_size(), serialized.len());

        let deserialized = FileData::deserialize_from(&serialized).unwrap();

        assert_eq!(file_data, deserialized);
    }

    #[test]
    fn test_transfer_response_ser_de() {
        let response = TransferResponseCode::Ok;

        let serialized = response.serialize_to();

        assert_eq!(TransferResponseCode::serialized_size(), serialized.len());

        let deserialized = TransferResponseCode::deserialize_from(&serialized).unwrap();

        assert_eq!(response, deserialized);
    }
}
