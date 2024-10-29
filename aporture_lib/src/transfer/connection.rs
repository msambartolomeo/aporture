use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::{JoinHandle, JoinSet};

use crate::crypto::cipher::Cipher;
use crate::net::quic::QuicConnection;
use crate::pairing::PairInfo;

const RETRIES: usize = 10;

type AddressError = (crate::io::Error, SocketAddr);

fn options_factory(
    pair_info: &PairInfo,
) -> Result<JoinSet<Result<QuicConnection, AddressError>>, crate::io::Error> {
    let cipher = pair_info.cipher();
    let binding_sockets = pair_info.binding_sockets();
    let connecting_sockets = pair_info.connecting_sockets();

    let mut set = JoinSet::new();

    for id in connecting_sockets {
        let socket = id.local_socket.try_clone()?;
        let destination = id.peer_address;
        let address = id.self_address;

        let fut = connect(socket, destination, address, Arc::clone(&cipher));

        set.spawn(fut);
    }

    for id in binding_sockets {
        let socket = id.local_socket.try_clone()?;
        let destination = id.peer_address;
        let address = id.self_address;

        let fut = bind(socket, destination, address, Arc::clone(&cipher));

        set.spawn(fut);
    }

    Ok(set)
}

pub async fn find(pair_info: &mut PairInfo) -> QuicConnection {
    for _ in 0..RETRIES {
        let Ok(mut options) = options_factory(pair_info) else {
            break;
        };

        loop {
            match options.join_next().await {
                Some(Ok(Ok(peer))) => {
                    // NOTE: Drop fallback if unused
                    drop(pair_info.fallback());

                    log::info!("Connected on {}", peer.address());

                    return peer;
                }
                Some(Ok(Err((e, a)))) => {
                    log::warn!("Could not connect to peer from ip {a}: {e}");
                    continue;
                }
                Some(_) => continue,
                None => break,
            }
        }
    }

    log::info!("Timeout waiting for peer connection, using server fallback");
    pair_info
        .fallback()
        .expect("Connection to server must exist")
        .add_cipher(pair_info.cipher());

    todo!();
}

pub async fn bind(
    socket: UdpSocket,
    destination: SocketAddr,
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<QuicConnection, AddressError> {
    log::info!(
        "Waiting for peer on {}, port {}; Peer address is {destination}",
        a.ip(),
        a.port()
    );

    let s = socket.try_clone().map_err(|e| (e.into(), a))?;
    let handle = keepalive(s, destination);

    let timeout = tokio::time::timeout(
        Duration::from_secs(10),
        QuicConnection::server(destination, socket, cipher, handle),
    );

    let peer = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e, a))?;

    Ok(peer)
}

pub async fn connect(
    socket: UdpSocket,
    a: SocketAddr,
    source: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<QuicConnection, AddressError> {
    log::info!(
        "Trying to connect to peer on {}, port {}; My address is {source}",
        a.ip(),
        a.port()
    );

    let s = socket.try_clone().map_err(|e| (e.into(), a))?;
    let handle = keepalive(s, a);

    let timeout = tokio::time::timeout(
        Duration::from_secs(10),
        QuicConnection::client(a, socket, cipher, handle),
    );

    let peer = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e, a))?;

    Ok(peer)
}

fn keepalive(socket: UdpSocket, peer: SocketAddr) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let _ = socket.send_to(b"ka", peer);
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    })
}
