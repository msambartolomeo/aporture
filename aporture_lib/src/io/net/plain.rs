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

        let message = Message::new(&mut serialized);

        let mut buf = message.into_buf();

        self.stream.write_all_buf(&mut buf).await?;

        Ok(())
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        if let Some(mut buffer) = P::buffer() {
            let message = Message::new(&mut buffer);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream.read_buf(&mut buf).await?;
            }

            let n = buf.consume()?;

            Ok(P::deserialize_from(&buffer[..n])?)
        } else {
            let mut buffer = vec![0; u16::MAX as usize];

            let message = Message::new(&mut buffer);

            let mut buf = message.into_buf();

            while buf.has_remaining_mut() {
                self.stream.read_buf(&mut buf).await?;
            }

            let n = buf.consume()?;

            Ok(P::deserialize_from(&buffer[..n])?)
        }
    }
}
