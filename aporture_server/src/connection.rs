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
    let mut buf = [0; 1024];

    match connection.socket.read(&mut buf).await {
        // socket closed
        Ok(0) => return,
        Ok(n) => n,
        Err(e) => {
            log::error!("failed to read from socket; err = {e:?}");
            return;
        }
    };

    let hello: AporturePairingProtocol =
        serde_bencode::de::from_bytes(&buf).expect("Response is valid APP");

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
