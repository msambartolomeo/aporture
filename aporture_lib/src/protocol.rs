use std::net::SocketAddr;

use generic_array::typenum::Unsigned;
use generic_array::{ArrayLength, GenericArray};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Receiver = 1,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
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
    #[serde_as(as = "Bytes")]
    pub timestamp: [u8; 16],
}

impl Default for KeyConfirmationPayload {
    fn default() -> Self {
        Self {
            tag: b"aporture".to_owned(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .expect("Now is after unix epoch")
                .as_millis()
                .to_be_bytes(),
        }
    }
}

pub trait Parser: Serialize + for<'a> Deserialize<'a> {
    type SerializedSize: ArrayLength;

    fn buffer() -> GenericArray<u8, Self::SerializedSize> {
        GenericArray::default()
    }

    fn serialized_size() -> usize {
        <Self::SerializedSize as Unsigned>::to_usize()
    }

    fn serialize_to(&self) -> Vec<u8> {
        serde_bencode::to_bytes(self)
            .inspect_err(|e| log::error!("Unknown error when serializing type {e}"))
            .expect("Serialization should not fail because the type is valid")
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, serde_bencode::Error> {
        serde_bencode::from_bytes(buffer)
    }
}

impl Parser for PairKind {
    type SerializedSize = generic_array::typenum::U3;
}

impl Parser for Hello {
    type SerializedSize = generic_array::typenum::U67;
}

impl Parser for ResponseCode {
    type SerializedSize = generic_array::typenum::U3;
}

impl Parser for KeyExchangePayload {
    type SerializedSize = generic_array::typenum::U36;
}

impl Parser for KeyConfirmationPayload {
    type SerializedSize = generic_array::typenum::U47;
}

impl Parser for SocketAddr {
    type SerializedSize = generic_array::typenum::U11;
}

// TODO: Remove and replace with sending many elements of P
impl<P: Parser> Parser for Vec<P> {
    type SerializedSize = generic_array::typenum::U3;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_response_ser_de() {
        let response = ResponseCode::Ok;

        let serialized = response.serialize_to();

        assert_eq!(ResponseCode::serialized_size(), serialized.len());

        let deserialized = ResponseCode::deserialize_from(&serialized).unwrap();

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
    fn test_key_confirmation_ser_de() {
        let payload = KeyConfirmationPayload::default();

        let serialized = payload.serialize_to();

        assert_eq!(KeyConfirmationPayload::serialized_size(), serialized.len());

        let deserialized = KeyConfirmationPayload::deserialize_from(&serialized).unwrap();

        assert_eq!(payload, deserialized);
    }

    #[test]
    fn test_address_ser_de() {
        let address = SocketAddr::from(([0, 0, 0, 0], 0));

        let serialized = address.serialize_to();

        assert_eq!(SocketAddr::serialized_size(), serialized.len());

        let deserialized = SocketAddr::deserialize_from(&serialized).unwrap();

        assert_eq!(address, deserialized);
    }
}
