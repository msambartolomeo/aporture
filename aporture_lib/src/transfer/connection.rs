use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::{JoinHandle, JoinSet};

use crate::crypto::cert::{Certificate, CertificateKey};
use crate::crypto::cipher::Cipher;
use crate::net::quic::QuicConnection;
use crate::pairing::PairInfo;

const RETRIES: usize = 5;

type AddressError = (crate::io::Error, SocketAddr);

fn options_factory(
    pair_info: &PairInfo,
) -> Result<JoinSet<Result<QuicConnection, AddressError>>, crate::io::Error> {
    let binding_sockets = pair_info.binding_sockets();
    let connecting_sockets = pair_info.connecting_sockets();

    let mut set = JoinSet::new();

    for id in connecting_sockets {
        let cipher = pair_info.cipher();
        let peer_cert = pair_info.peer_certificate();
        let socket = id.local_socket.try_clone()?;
        let destination = id.peer_address;
        let address = id.self_address;

        let fut = connect(socket, destination, address, cipher, peer_cert);

        set.spawn(fut);
    }

    for id in binding_sockets {
        let cipher = pair_info.cipher();
        let self_cert = pair_info.self_certificate();
        let socket = id.local_socket.try_clone()?;
        let destination = id.peer_address;
        let address = id.self_address;

        let fut = bind(socket, destination, address, cipher, self_cert);

        set.spawn(fut);
    }

    Ok(set)
}

pub async fn find(pair_info: &mut PairInfo) -> Option<QuicConnection> {
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

                    return Some(peer);
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

    None
}

pub async fn bind(
    socket: UdpSocket,
    destination: SocketAddr,
    a: SocketAddr,
    cipher: Arc<Cipher>,
    certificate: CertificateKey,
) -> Result<QuicConnection, AddressError> {
    log::info!(
        "Waiting for peer on {}, port {}; Peer address is {destination}",
        a.ip(),
        a.port()
    );

    let s = socket.try_clone().map_err(|e| (e.into(), a))?;
    let handle = keepalive(s, destination);

    let timeout = tokio::time::timeout(
        Duration::from_secs(15),
        QuicConnection::server(destination, socket, cipher, certificate, handle),
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
    certificate: Certificate,
) -> Result<QuicConnection, AddressError> {
    log::info!(
        "Trying to connect to peer on {}, port {}; My address is {source}",
        a.ip(),
        a.port()
    );

    let s = socket.try_clone().map_err(|e| (e.into(), a))?;
    let handle = keepalive(s, a);

    let timeout = tokio::time::timeout(
        Duration::from_secs(15),
        QuicConnection::client(a, socket, cipher, certificate, handle),
    );

    let peer = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e, a))?;

    Ok(peer)
}

fn keepalive(socket: UdpSocket, peer: SocketAddr) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = socket.send_to(b"ka", peer);
        let _ = socket.send_to(b"ka", peer);
        let _ = socket.send_to(b"ka", peer);
        let _ = socket.send_to(b"ka", peer);

        loop {
            let _ = socket.send_to(b"ka", peer);
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    })
}
