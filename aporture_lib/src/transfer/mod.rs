use crate::net::NetworkPeer;
use crate::pairing::PairInfo;
use crate::protocol::{FileData, TransferResponseCode};

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use directories::UserDirs;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

mod error;
pub use error::{Receive as ReceiveError, Send as SendError};

pub async fn send_file(file: &Path, pair_info: &mut PairInfo) -> Result<(), error::Send> {
    // TODO: Find better alternative
    // NOTE: Sleep to give time to receiver to bind
    tokio::time::sleep(Duration::from_secs(1)).await;

    let options = pair_info
        .addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, a| {
            set.spawn(connect(a));
            set
        });

    let mut peer = find_peer(options, pair_info).await;

    peer.add_cipher(pair_info.cipher().clone());

    // TODO: Buffer file
    let mut file = tokio::fs::read(file).await?;

    let hash = blake3::hash(&file);

    let file_data = FileData {
        hash: *hash.as_bytes(),
        file_size: file.len().to_be_bytes(),
        file_name: PathBuf::new(),
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
    let dest = dest.unwrap_or_else(|| {
        UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .expect("Valid Download Directory")
    });

    let options = pair_info
        .bind_addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, (b, a)| {
            set.spawn(bind(b, a));
            set
        });

    let mut peer = find_peer(options, pair_info).await;

    peer.add_cipher(pair_info.cipher().clone());

    let file_data = peer.read_ser_enc::<FileData>().await?;
    // TODO: Use file name if exists

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
    mut options: JoinSet<Result<(TcpStream, SocketAddr), std::io::Error>>,
    pair_info: &mut PairInfo,
) -> NetworkPeer {
    loop {
        match options.join_next().await {
            Some(Ok(Ok((peer, address)))) => {
                log::info!("Peer connected to {}", address);

                // NOTE: Drop fallback if unused
                drop(pair_info.fallback());

                break NetworkPeer::new(peer);
            }
            Some(_) => continue,
            None => {
                log::info!("Using server fallback");
                break pair_info
                    .fallback()
                    .expect("Connection to server must exist");
            }
        }
    }
}

async fn bind(
    bind_address: SocketAddr,
    full_address: SocketAddr,
) -> Result<(TcpStream, SocketAddr), std::io::Error> {
    log::info!(
        "Trying bind to {} on port {}",
        full_address.ip(),
        full_address.port(),
    );

    let listener = TcpListener::bind(bind_address).await?;

    let (peer, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept()).await??;

    Ok((peer, full_address))
}

async fn connect(a: SocketAddr) -> Result<(TcpStream, SocketAddr), std::io::Error> {
    log::info!("Trying connection to {} on port {}", a.ip(), a.port());

    Ok((TcpStream::connect(a).await?, a))
}
