use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::protocol::{AporturePairingProtocol, PairKind, ResponseCode};

pub struct Connection {
    pub socket: TcpStream,
    pub address: SocketAddr,
}

impl From<(TcpStream, SocketAddr)> for Connection {
    fn from((socket, address): (TcpStream, SocketAddr)) -> Self {
        Self { socket, address }
    }
}

pub async fn handle_connection(
    mut connection: Connection,
    map: Arc<Mutex<HashMap<[u8; 64], Connection>>>,
) {
    let mut buf = [0; AporturePairingProtocol::serialized_size()];

    if let Err(e) = connection.socket.read_exact(&mut buf).await {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => {
                log::warn!("Content does not match APP hello message length: {e}")
            }
            _ => log::warn!("Error reading APP hello message: {e}"),
        }
        return;
    };

    let Ok(hello) = serde_bencode::from_bytes::<AporturePairingProtocol>(&buf) else {
        log::warn!("Hello message does not match APP hello");
        return;
    };

    let mut map = map.lock().await;

    match hello.kind {
        PairKind::Sender => {
            log::info!("recieved hello from sender");
            map.insert(hello.pair_id, connection);
        }
        PairKind::Reciever => {
            let mut receiver = connection;

            log::info!("recieved hello from reciever");
            let mut sender = map.remove(&hello.pair_id).expect("Sender already arrived");

            // NOTE: Drop map to allow other connections
            drop(map);

            let response = if receiver.address.ip() == sender.address.ip() {
                ResponseCode::OkSamePublicIP
            } else {
                ResponseCode::Ok
            };

            let response =
                serde_bencode::to_bytes(&response).expect("Response code can be turn into bencode");

            sender
                .socket
                .write_all(&response)
                .await
                .expect("No network errors sending response");

            receiver
                .socket
                .write_all(&response)
                .await
                .expect("No network errors sending response");

            log::info!("Starting bidirectional APP");

            // NOTE: Delegate talking between pairs, per protocol
            tokio::io::copy_bidirectional(&mut sender.socket, &mut receiver.socket)
                .await
                .expect("No network errors in pairing");

            log::info!("Finished pairing");
        }
    }
}
