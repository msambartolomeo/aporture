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

#[allow(async_fn_in_trait)]
pub trait SerdeIO {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error>;
    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error>;
}

#[cfg(feature = "full")]
#[allow(async_fn_in_trait)]
pub trait EncryptedSerdeIO: SerdeIO {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error>;
    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), crate::io::Error>;
    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error>;
    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<usize, crate::io::Error>;
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
