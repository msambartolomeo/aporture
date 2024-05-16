use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use directories::UserDirs;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use crate::crypto::Cipher;
use crate::net::crypto::EncryptedNetworkPeer;
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, KeyConfirmationPayload, TransferResponseCode};

mod error;
pub use error::{Receive as ReceiveError, Send as SendError};

pub async fn send_file(path: &Path, pair_info: &mut PairInfo) -> Result<(), error::Send> {
    let Some(file_name) = path.file_name() else {
        return Err(error::Send::Path);
    };

    // TODO: Find better alternative
    // NOTE: Sleep to give time to receiver to bind
    tokio::time::sleep(Duration::from_secs(1)).await;

    let options = pair_info
        .addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, a| {
            set.spawn(connect(a, pair_info.cipher()));
            set
        });

    let mut peer = find_peer(options, pair_info).await;

    // TODO: Buffer file
    let mut file = tokio::fs::read(path).await?;

    let hash = blake3::hash(&file);

    let file_data = FileData {
        hash: *hash.as_bytes(),
        file_size: file.len().to_be_bytes(),
        // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
        file_name: file_name.to_owned(),
    };

    peer.write_ser_enc(&file_data).await?;
    peer.write_enc(&mut file).await?;

    let response = peer.read_ser_enc::<TransferResponseCode>().await?;

    match response {
        TransferResponseCode::Ok => {
            log::info!("File transfered correctly");
            Ok(())
        }
        TransferResponseCode::HashMismatch => {
            log::error!("Hash mismatch in file transfer");
            Err(error::Send::HashMismatch)
        }
    }
}

pub async fn receive_file(
    dest: Option<PathBuf>,
    pair_info: &mut PairInfo,
) -> Result<PathBuf, error::Receive> {
    let mut dest = dest.unwrap_or_else(|| {
        UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .expect("Valid Download Directory")
    });

    let options = pair_info
        .bind_addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, (b, a)| {
            set.spawn(bind(b, a, pair_info.cipher()));
            set
        });

    let mut peer = find_peer(options, pair_info).await;

    let file_data = peer.read_ser_enc::<FileData>().await?;

    if dest.is_dir() {
        dest.push(file_data.file_name);
    }

    let mut file = vec![0; usize::from_be_bytes(file_data.file_size)];

    peer.read_enc(&mut file).await?;

    let hash = blake3::hash(&file);

    let response = if hash == file_data.hash {
        TransferResponseCode::Ok
    } else {
        TransferResponseCode::HashMismatch
    };

    peer.write_ser_enc(&response).await?;

    tokio::fs::write(&dest, file).await?;

    Ok(dest)
}

async fn find_peer(
    mut options: JoinSet<Result<EncryptedNetworkPeer, (crate::net::Error, SocketAddr)>>,
    pair_info: &mut PairInfo,
) -> EncryptedNetworkPeer {
    loop {
        match options.join_next().await {
            Some(Ok(Ok(peer))) => {
                // NOTE: Drop fallback if unused
                drop(pair_info.fallback());

                break peer;
            }
            Some(Ok(Err((e, a)))) => {
                log::warn!("Could not connect to peer from ip {a}: {e}");
                continue;
            }
            Some(_) => continue,
            None => {
                log::info!("Timeout waiting for peer connection, using server fallback");
                break pair_info
                    .fallback()
                    .expect("Connection to server must exist")
                    .add_cipher(pair_info.cipher());
            }
        }
    }
}

async fn bind(
    bind_address: SocketAddr,
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<EncryptedNetworkPeer, (crate::net::Error, SocketAddr)> {
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

async fn connect(
    a: SocketAddr,
    cipher: Arc<Cipher>,
) -> Result<EncryptedNetworkPeer, (crate::net::Error, SocketAddr)> {
    log::info!("Trying to connect to peer on {}, port {}", a.ip(), a.port());

    let stream = TcpStream::connect(a).await.map_err(|e| (e.into(), a))?;

    let peer = EncryptedNetworkPeer::new(stream, cipher);

    exchange_hello(peer, a).await
}

async fn exchange_hello(
    mut peer: EncryptedNetworkPeer,
    a: SocketAddr,
) -> Result<EncryptedNetworkPeer, (crate::net::Error, SocketAddr)> {
    let hello = KeyConfirmationPayload::default();

    peer.write_ser_enc(&hello).await.map_err(|e| (e, a))?;

    let peer_hello = peer
        .read_ser_enc::<KeyConfirmationPayload>()
        .await
        .map_err(|e| (e, a))?;

    let difference = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("Now is after unix epoch")
        .checked_sub(peer_hello.timestamp);

    if &peer_hello.tag == b"aporture" && difference.is_some_and(|s| s.as_secs() < 11) {
        log::info!("Connected to peer on {}", a);
        Ok(peer)
    } else {
        Err((crate::net::Error::Custom("Invalid tag and timestamp"), a))
    }
}
