use crate::io::Error;
use std::future::Future;

use generic_array::{ArrayLength, GenericArray};
use serde::{Deserialize, Serialize};

pub trait Parser: Serialize + for<'a> Deserialize<'a> {
    type MaximumSerializedSize: ArrayLength;

    fn buffer() -> Option<GenericArray<u8, Self::MaximumSerializedSize>>;

    fn serialized_size() -> Option<usize>;

    fn serialize_to(&self) -> Vec<u8> {
        serde_bencode::to_bytes(self)
            .inspect_err(|e| log::error!("Unknown error when serializing type {e}"))
            .expect("Serialization should not fail because the type is valid")
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, serde_bencode::Error> {
        serde_bencode::from_bytes(buffer)
    }
}

pub trait SerdeIO {
    fn read_ser<P: Parser + Sync>(&mut self) -> impl Future<Output = Result<P, Error>> + Send;
    fn write_ser<P>(&mut self, input: &P) -> impl Future<Output = Result<(), Error>> + Send
    where
        P: Parser + Sync;
}

#[cfg(feature = "full")]
pub trait EncryptedSerdeIO: SerdeIO {
    fn read_enc(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send;
    fn read_ser_enc<P: Parser + Sync>(&mut self) -> impl Future<Output = Result<P, Error>> + Send;
    fn write_enc(&mut self, input: &mut [u8]) -> impl Future<Output = Result<(), Error>> + Send;
    fn write_ser_enc<P>(&mut self, input: &P) -> impl Future<Output = Result<(), Error>> + Send
    where
        P: Parser + Sync;
}

#[macro_export]
macro_rules! parse {
    ($type:ty) => {
        impl Parser for $type {
            type MaximumSerializedSize = generic_array::typenum::U0;

            fn buffer() -> Option<GenericArray<u8, Self::MaximumSerializedSize>> {
                None
            }

            fn serialized_size() -> Option<usize> {
                None
            }
        }
    };
    ($type:ty, size: $size:ty) => {
        impl Parser for $type {
            type MaximumSerializedSize = $size;

            fn buffer() -> Option<GenericArray<u8, Self::MaximumSerializedSize>> {
                Some(GenericArray::default())
            }

            fn serialized_size() -> Option<usize> {
                Some(<Self::MaximumSerializedSize as Unsigned>::to_usize())
            }
        }
    };
}

impl<P: Parser> Parser for Vec<P> {
    type MaximumSerializedSize = generic_array::typenum::U0;

    fn buffer() -> Option<GenericArray<u8, Self::MaximumSerializedSize>> {
        None
    }

    fn serialized_size() -> Option<usize> {
        None
    }
}
