use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use aporture::net::NetworkPeer;
use aporture::parser::SerdeIO;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, MutexGuard};

use aporture::protocol::{Hello, PairKind, PairingResponseCode};

pub struct Connection {
    pub stream: NetworkPeer,
    pub address: SocketAddr,
}

impl From<(TcpStream, SocketAddr)> for Connection {
    fn from((stream, address): (TcpStream, SocketAddr)) -> Self {
        let peer = NetworkPeer::new(stream);
        Self {
            stream: peer,
            address,
        }
    }
}

pub async fn handle_connection(
    mut connection: Connection,
    map: Arc<Mutex<HashMap<[u8; 32], Connection>>>,
) {
    let hello = match connection.stream.read_ser::<Hello>().await {
        Ok(hello) => hello,
        Err(e) => {
            log::warn!(
                "Error reading hello message from {}: {e}",
                connection.address
            );

            let _ = connection
                .stream
                .write_ser(&PairingResponseCode::MalformedMessage)
                .await;

            return;
        }
    };

    if hello.version != aporture::protocol::PROTOCOL_VERSION {
        log::warn!("Not supported protocol version");

        let _ = connection
            .stream
            .write_ser(&PairingResponseCode::UnsupportedVersion)
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
    log::info!("received hello from sender from {}", connection.address);
    map.insert(id, connection);

    drop(map);
}

async fn handle_receiver(
    connection: Connection,
    id: &[u8],
    mut map: MutexGuard<'_, HashMap<[u8; 32], Connection>>,
) {
    log::info!("received hello from receiver from {}", connection.address);
    let mut receiver = connection;
    let Some(mut sender) = map.remove(id) else {
        drop(map);

        log::warn!("Sender must arrive first and has not");

        let _ = receiver
            .stream
            .write_ser(&PairingResponseCode::NoPeer)
            .await;

        return;
    };

    // NOTE: Drop map to allow other connections
    drop(map);

    let response = if receiver.address.ip() == sender.address.ip() {
        PairingResponseCode::OkSamePublicIP
    } else {
        PairingResponseCode::Ok
    };

    if sender.stream.write_ser(&response).await.is_err() {
        log::warn!("Connection closed from sender");

        let _ = receiver
            .stream
            .write_ser(&PairingResponseCode::NoPeer)
            .await;

        return;
    }

    if receiver.stream.write_ser(&response).await.is_err() {
        log::warn!("Connection closed from receiver");

        let _ = sender.stream.write_ser(&PairingResponseCode::NoPeer).await;

        return;
    }

    log::info!("Starting bidirectional APP");

    // NOTE: Delegate talking between pairs
    let result =
        tokio::io::copy_bidirectional(sender.stream.inner(), receiver.stream.inner()).await;

    if result.is_err() {
        log::warn!("Error during pairing");
        return;
    }

    log::info!("Finished pairing");
}
