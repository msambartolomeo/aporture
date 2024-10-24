use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::peer::Peer;

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

impl Peer for NetworkPeer {
    fn writer(&mut self) -> impl AsyncWriteExt + Unpin {
        &mut self.stream
    }

    fn reader(&mut self) -> impl AsyncReadExt + Unpin {
        &mut self.stream
    }
}
