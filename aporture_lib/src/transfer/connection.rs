use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;

use crate::crypto::cipher::Cipher;
use crate::net::quic::QuicConnection;
use crate::pairing::PairInfo;

const RETRIES: usize = 10;

fn options_factory(
    pair_info: &PairInfo,
) -> Result<JoinSet<Result<QuicConnection, (crate::io::Error, SocketAddr)>>, crate::io::Error> {
    let cipher = pair_info.cipher();
    let sockets = pair_info.sockets();
    let addresses = pair_info.pair_addresses();

    let mut set = JoinSet::new();

    for a in addresses {
        let fut = connect(*a, Arc::clone(&cipher));

        set.spawn(fut);
    }

    for (s, a) in &sockets {
        let fut = bind(s.try_clone()?, *a, Arc::clone(&cipher));

        set.spawn(fut);
    }

    Ok(set)
}

pub async fn find(pair_info: &mut PairInfo) -> QuicConnection {
    for _ in 0..RETRIES {
        let Ok(mut options) = options_factory(&pair_info) else {
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
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<QuicConnection, (crate::io::Error, SocketAddr)> {
    log::info!("Waiting for peer on {}, port {}", a.ip(), a.port());

    let timeout = tokio::time::timeout(
        Duration::from_secs(10),
        QuicConnection::server(a, socket, cipher),
    );

    let peer = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e, a))?;

    Ok(peer)
}

pub async fn connect(
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<QuicConnection, (crate::io::Error, SocketAddr)> {
    log::info!("Trying to connect to peer on {}, port {}", a.ip(), a.port());

    let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).map_err(|e| (e.into(), a))?;

    let timeout = tokio::time::timeout(
        Duration::from_secs(10),
        QuicConnection::client(socket, cipher, a),
    );

    let peer = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e, a))?;

    Ok(peer)
}
