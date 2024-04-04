use std::io::{Read, Write};

use thiserror::Error;

use crate::crypto::{Cipher, DecryptError};
use crate::protocol::Parser;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Network failure: {0}")]
    IO(std::io::Error),

    #[error("Serde error: {0}")]
    SerDe(serde_bencode::Error),

    #[error("Cipher error: {0}")]
    Cipher(DecryptError),
}

pub fn write_ser<P: Parser>(stream: &mut impl Write, input: &P) -> Result<(), Error> {
    let in_buf = input.serialize_to();

    stream.write_all(&in_buf).map_err(Error::IO)
}

pub fn write_ser_enc<P: Parser>(
    stream: &mut impl Write,
    cipher: &mut Cipher,
    input: &P,
) -> Result<(), Error> {
    let mut buf = input.serialize_to();

    write_enc(stream, cipher, &mut buf)
}

pub fn write_enc(
    stream: &mut impl Write,
    cipher: &mut Cipher,
    input: &mut [u8],
) -> Result<(), Error> {
    let (nonce, tag) = cipher.encrypt(input);

    stream.write_all(&nonce).map_err(Error::IO)?;
    stream.write_all(&input).map_err(Error::IO)?;
    stream.write_all(&tag).map_err(Error::IO)?;

    Ok(())
}

pub fn read_ser<P: Parser>(stream: &mut impl Read) -> Result<P, Error> {
    let mut buffer = P::buffer();

    stream.read_exact(&mut buffer).map_err(Error::IO)?;

    P::deserialize_from(&buffer).map_err(Error::SerDe)
}

pub fn read_ser_enc<P: Parser>(stream: &mut impl Read, cipher: &mut Cipher) -> Result<P, Error> {
    let mut buffer = P::buffer();

    read_enc(stream, cipher, &mut buffer)?;

    P::deserialize_from(&buffer).map_err(Error::SerDe)
}

pub fn read_enc(
    stream: &mut impl Read,
    cipher: &mut Cipher,
    buffer: &mut [u8],
) -> Result<(), Error> {
    let mut nonce = [0; 12];
    let mut tag = [0; 16];

    stream.read_exact(&mut nonce).map_err(Error::IO)?;
    stream.read_exact(buffer).map_err(Error::IO)?;
    stream.read_exact(&mut tag).map_err(Error::IO)?;

    cipher.decrypt(buffer, &nonce, &tag).map_err(Error::Cipher)
}
