use bytes::BufMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::parser::{Parser, SerdeIO};

use super::message::Message;

#[derive(Debug)]
pub struct NetworkPeer {
    stream: TcpStream,
}

impl NetworkPeer {
    pub const fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    pub fn inner(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

impl SerdeIO for NetworkPeer {
    async fn write_ser<P: Parser + Sync>(&mut self, input: &P) -> Result<(), crate::io::Error> {
        let mut serialized = input.serialize_to();

        let message = Message::new(&mut serialized, None);

        let mut buf = message.into_buf();

        self.stream.write_all_buf(&mut buf).await?;

        // let in_buf = input.serialize_to();

        // self.stream.write_all(&in_buf.len().to_be_bytes()).await?;

        // self.stream.write_all(&in_buf).await?;

        Ok(())
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new(&mut buffer, None);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream.read_buf(&mut buf).await?;
            }

            let n = buf.consume(None).unwrap();

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new(&mut buffer, None);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream.read_buf(&mut buf).await?;
            }

            let n = buf.consume(None).unwrap();

            Ok(P::deserialize_from(&buffer[..n])?)
        }

        // let mut length = [0; 8];

        // self.stream.read_exact(&mut length).await?;

        // let length = usize::from_be_bytes(length);

        // if length == P::serialized_size() {
        //     let mut buffer = P::buffer();

        //     self.stream.read_exact(&mut buffer).await?;

        //     let deserialized = P::deserialize_from(&buffer)?;

        //     Ok(deserialized)
        // } else {
        //     let mut buffer = vec![0; length];

        //     self.stream.read_exact(&mut buffer).await?;

        //     let deserialized = P::deserialize_from(&buffer)?;

        //     Ok(deserialized)
        // }
    }
}
