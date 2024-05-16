use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::parser::{Parser, SerdeIO};

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
        let in_buf = input.serialize_to();

        self.stream.write_all(&in_buf.len().to_be_bytes()).await?;

        self.stream.write_all(&in_buf).await?;

        Ok(())
    }

    async fn read_ser<P: Parser + Sync>(&mut self) -> Result<P, crate::io::Error> {
        let mut length = [0; 8];

        self.stream.read_exact(&mut length).await?;

        let length = usize::from_be_bytes(length);

        if length == P::serialized_size() {
            let mut buffer = P::buffer();

            self.stream.read_exact(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        } else {
            let mut buffer = vec![0; length];

            self.stream.read_exact(&mut buffer).await?;

            let deserialized = P::deserialize_from(&buffer)?;

            Ok(deserialized)
        }
    }
}
