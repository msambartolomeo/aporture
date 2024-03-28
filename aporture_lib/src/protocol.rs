use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, Bytes};

pub trait Parser: Serialize + for<'a> Deserialize<'a> {
    const SERIALIZED_SIZE: usize;

    fn serialize_to(&self) -> Vec<u8> {
        serde_bencode::to_bytes(self)
            .inspect_err(|e| log::error!("Unknown error when serializing type {e}"))
            .expect("Serialization should not fail because the type is valid")
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, serde_bencode::Error> {
        serde_bencode::from_bytes(buffer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum PairKind {
    Sender = 0,
    Receiver = 1,
}

impl Parser for PairKind {
    const SERIALIZED_SIZE: usize = 3;
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

impl Parser for Hello {
    const SERIALIZED_SIZE: usize = 67;
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

impl Parser for ResponseCode {
    const SERIALIZED_SIZE: usize = 3;
}

#[serde_as]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyExchangePayload(#[serde_as(as = "Bytes")] pub [u8; 33]);

impl Parser for KeyExchangePayload {
    const SERIALIZED_SIZE: usize = 36;
}

impl Parser for SocketAddr {
    const SERIALIZED_SIZE: usize = 11;
}

impl<P: Parser> Parser for Vec<P> {
    const SERIALIZED_SIZE: usize = panic!("Variable Size");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_response_ser_de() {
        let response = ResponseCode::Ok;

        let serialized = response.serialize_to();

        assert_eq!(ResponseCode::SERIALIZED_SIZE, serialized.len());

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

        assert_eq!(Hello::SERIALIZED_SIZE, serialized.len());

        let deserialized = Hello::deserialize_from(&serialized).unwrap();

        assert_eq!(hello, deserialized);
    }

    #[test]
    fn test_pair_kind_ser_de() {
        let pair = PairKind::Sender;

        let serialized = pair.serialize_to();

        assert_eq!(PairKind::SERIALIZED_SIZE, serialized.len());

        let deserialized = PairKind::deserialize_from(&serialized).unwrap();

        assert_eq!(pair, deserialized);
    }

    #[test]
    fn test_key_exchange_ser_de() {
        let key_exchange = KeyExchangePayload([0; 33]);

        let serialized = key_exchange.serialize_to();

        assert_eq!(KeyExchangePayload::SERIALIZED_SIZE, serialized.len());

        let deserialized = KeyExchangePayload::deserialize_from(&serialized).unwrap();

        assert_eq!(key_exchange, deserialized);
    }

    #[test]
    fn test_address_ser_de() {
        let address = SocketAddr::from(([0, 0, 0, 0], 0));

        let serialized = address.serialize_to();

        assert_eq!(SocketAddr::SERIALIZED_SIZE, serialized.len());

        let deserialized = SocketAddr::deserialize_from(&serialized).unwrap();

        assert_eq!(address, deserialized);
    }
}
