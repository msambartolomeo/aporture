use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard};

use crate::protocol::{APPHello, BencodeSerDe, PairKind, ResponseCode};

const SUPPORTED_VERSION: u8 = 1;

pub struct Connection {
    pub socket: TcpStream,
    pub address: SocketAddr,
}

impl Connection {
    pub async fn send_response(&mut self, response: ResponseCode) -> Result<(), std::io::Error> {
        self.socket.write_all(&response.serialize()).await
    }
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
    let mut buf = [0; APPHello::SERIALIZED_SIZE];

    if let Err(e) = connection.socket.read_exact(&mut buf).await {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            log::warn!("Content does not match APP hello message length: {e}");
            let _ = connection
                .send_response(ResponseCode::MalformedMessage)
                .await;
        } else {
            log::warn!("Error reading APP hello message: {e}");
        }

        return;
    };

    let Ok(hello) = APPHello::deserialize_from(&buf) else {
        log::warn!("Hello message does not match APP hello");
        let _ = connection
            .send_response(ResponseCode::MalformedMessage)
            .await;
        return;
    };

    if hello.version != SUPPORTED_VERSION {
        log::warn!("Not supported protocol version");

        let _ = connection
            .send_response(ResponseCode::UnsupportedVersion)
            .await;

        return;
    }

    let map = map.lock().await;

    match hello.kind {
        PairKind::Sender => handle_sender(connection, hello.pair_id, map),
        PairKind::Reciever => handle_receiver(connection, &hello.pair_id, map).await,
    }
}

fn handle_sender(
    connection: Connection,
    id: [u8; 64],
    mut map: MutexGuard<'_, HashMap<[u8; 64], Connection>>,
) {
    log::info!("recieved hello from sender");
    map.insert(id, connection);

    drop(map);
}

async fn handle_receiver<'a>(
    connection: Connection,
    id: &[u8],
    mut map: MutexGuard<'a, HashMap<[u8; 64], Connection>>,
) {
    log::info!("recieved hello from reciever");
    let mut receiver = connection;
    let Some(mut sender) = map.remove(id) else {
        log::warn!("Sender must arrive first and has not");

        let _ = receiver.send_response(ResponseCode::NoPeer).await;

        return;
    };

    // NOTE: Drop map to allow other connections
    drop(map);

    let response = if receiver.address.ip() == sender.address.ip() {
        ResponseCode::OkSamePublicIP
    } else {
        ResponseCode::Ok
    };

    if sender.send_response(response).await.is_err() {
        log::warn!("Connection closed from sender");

        let _ = receiver.send_response(ResponseCode::NoPeer).await;

        return;
    }

    if receiver.send_response(response).await.is_err() {
        log::warn!("Connection closed from receiver");

        let _ = sender.send_response(ResponseCode::NoPeer).await;

        return;
    }

    log::info!("Starting bidirectional APP");

    // NOTE: Delegate talking between pairs
    let result = tokio::io::copy_bidirectional(&mut sender.socket, &mut receiver.socket).await;
    if result.is_err() {
        log::warn!("Error during pairing");
        return;
    }

    log::info!("Finished pairing");
}
