use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard};

use aporture::protocol::{Hello, PairKind, PairingResponseCode, Parser};

pub struct Connection {
    pub socket: TcpStream,
    pub address: SocketAddr,
}

impl Connection {
    pub async fn send_response(
        &mut self,
        response: PairingResponseCode,
    ) -> Result<(), std::io::Error> {
        let response = response.serialize_to();

        self.socket.write_all(&response.len().to_be_bytes()).await?;
        self.socket.write_all(&response).await
    }
}

impl From<(TcpStream, SocketAddr)> for Connection {
    fn from((socket, address): (TcpStream, SocketAddr)) -> Self {
        Self { socket, address }
    }
}

pub async fn handle_connection(
    mut connection: Connection,
    map: Arc<Mutex<HashMap<[u8; 32], Connection>>>,
) {
    let mut length = [0; 8];

    if let Err(e) = connection.socket.read_exact(&mut length).await {
        log::warn!("No hello message length received: {e}");

        let _ = connection
            .send_response(PairingResponseCode::MalformedMessage)
            .await;
    }

    let mut buf = Hello::buffer();
    if buf.len() != usize::from_be_bytes(length) {
        log::warn!("Invalid hello message length");
        let _ = connection
            .send_response(PairingResponseCode::MalformedMessage)
            .await;

        return;
    }

    if let Err(e) = connection.socket.read_exact(&mut buf).await {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            log::warn!("Content does not match APP hello message length: {e}");
            let _ = connection
                .send_response(PairingResponseCode::MalformedMessage)
                .await;
        } else {
            log::warn!("Error reading APP hello message: {e}");
        }

        return;
    };

    let Ok(hello) = Hello::deserialize_from(&buf) else {
        log::warn!("Hello message does not match APP hello");
        let _ = connection
            .send_response(PairingResponseCode::MalformedMessage)
            .await;
        return;
    };

    if hello.version != aporture::protocol::PROTOCOL_VERSION {
        log::warn!("Not supported protocol version");

        let _ = connection
            .send_response(PairingResponseCode::UnsupportedVersion)
            .await;

        return;
    }

    let map = map.lock().await;

    match hello.kind {
        PairKind::Sender => handle_sender(connection, hello.pair_id, map),
        PairKind::Receiver => handle_receiver(connection, &hello.pair_id, map).await,
    }
}

fn handle_sender(
    connection: Connection,
    id: [u8; 32],
    mut map: MutexGuard<'_, HashMap<[u8; 32], Connection>>,
) {
    log::info!("received hello from sender");
    map.insert(id, connection);

    drop(map);
}

async fn handle_receiver<'a>(
    connection: Connection,
    id: &[u8],
    mut map: MutexGuard<'a, HashMap<[u8; 32], Connection>>,
) {
    log::info!("received hello from receiver");
    let mut receiver = connection;
    let Some(mut sender) = map.remove(id) else {
        drop(map);

        log::warn!("Sender must arrive first and has not");

        let _ = receiver.send_response(PairingResponseCode::NoPeer).await;

        return;
    };

    // NOTE: Drop map to allow other connections
    drop(map);

    let response = if receiver.address.ip() == sender.address.ip() {
        PairingResponseCode::OkSamePublicIP
    } else {
        PairingResponseCode::Ok
    };

    if sender.send_response(response).await.is_err() {
        log::warn!("Connection closed from sender");

        let _ = receiver.send_response(PairingResponseCode::NoPeer).await;

        return;
    }

    if receiver.send_response(response).await.is_err() {
        log::warn!("Connection closed from receiver");

        let _ = sender.send_response(PairingResponseCode::NoPeer).await;

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
