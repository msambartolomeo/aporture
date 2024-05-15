use generic_array::{typenum::Unsigned, ArrayLength, GenericArray};
use serde::{Deserialize, Serialize};

mod error;
pub use error::Error;

pub trait Parser: Serialize + for<'a> Deserialize<'a> {
    type MinimumSerializedSize: ArrayLength;

    #[must_use]
    fn buffer() -> GenericArray<u8, Self::MinimumSerializedSize> {
        GenericArray::default()
    }

    #[must_use]
    fn serialized_size() -> usize {
        <Self::MinimumSerializedSize as Unsigned>::to_usize()
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

#[allow(async_fn_in_trait)]
pub trait SerdeIO {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error>;
    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, Error>;
}

#[cfg(feature = "full")]
#[allow(async_fn_in_trait)]
pub trait EncryptedSerdeIO: SerdeIO {
    async fn write_ser_enc<P: Parser + Sync>(&mut self, input: &P) -> Result<(), Error>;
    async fn write_enc(&mut self, input: &mut [u8]) -> Result<(), Error>;
    async fn read_ser_enc<P: Parser + Sync>(&mut self) -> Result<P, Error>;
    async fn read_enc(&mut self, buffer: &mut [u8]) -> Result<(), Error>;
}
