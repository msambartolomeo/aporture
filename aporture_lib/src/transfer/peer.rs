use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use crate::crypto::cipher::Cipher;
use crate::net::EncryptedNetworkPeer;
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::TransferHello;

const RETRIES: usize = 10;

pub async fn find<F>(options_factory: F, pair_info: &mut PairInfo) -> EncryptedNetworkPeer
where
    F: Fn() -> JoinSet<Result<EncryptedNetworkPeer, (crate::io::Error, SocketAddr)>> + Send,
{
    for _ in 0..RETRIES {
        let mut options = options_factory();

        loop {
            match options.join_next().await {
                Some(Ok(Ok(peer))) => {
                    // NOTE: Drop fallback if unused
                    drop(pair_info.fallback());

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
        .add_cipher(pair_info.cipher())
}

pub async fn bind(
    bind_address: SocketAddr,
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<EncryptedNetworkPeer, (crate::io::Error, SocketAddr)> {
    log::info!("Waiting for peer on {}, port {}", a.ip(), a.port(),);

    let listener = TcpListener::bind(bind_address)
        .await
        .map_err(|e| (e.into(), a))?;

    let timeout = tokio::time::timeout(Duration::from_secs(10), listener.accept());

    let (stream, _) = timeout
        .await
        .map_err(|e| (std::io::Error::from(e).into(), a))?
        .map_err(|e| (e.into(), a))?;

    let peer = EncryptedNetworkPeer::new(stream, cipher);

    exchange_hello(peer, a).await
}

pub async fn connect(
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<EncryptedNetworkPeer, (crate::io::Error, SocketAddr)> {
    log::info!("Trying to connect to peer on {}, port {}", a.ip(), a.port());

    let stream = TcpStream::connect(a).await.map_err(|e| (e.into(), a))?;

    let peer = EncryptedNetworkPeer::new(stream, cipher);

    exchange_hello(peer, a).await
}

async fn exchange_hello(
    mut peer: EncryptedNetworkPeer,
    a: SocketAddr,
) -> Result<EncryptedNetworkPeer, (crate::io::Error, SocketAddr)> {
    let hello = TransferHello::default();

    peer.write_ser_enc(&hello).await.map_err(|e| (e, a))?;

    let peer_hello = peer
        .read_ser_enc::<TransferHello>()
        .await
        .map_err(|e| (e, a))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("Now is after unix epoch");

    let difference = timestamp
        .checked_sub(peer_hello.timestamp)
        .or_else(|| peer_hello.timestamp.checked_sub(timestamp));

    if &peer_hello.tag == b"aporture" && difference.is_some_and(|s| s.as_secs() < 15) {
        log::info!("Connected to peer on {}", a);
        Ok(peer)
    } else {
        log::error!("Invalid hello {hello:?} or difference {difference:?}");
        Err((crate::io::Error::Custom("Invalid tag and timestamp"), a))
    }
}
