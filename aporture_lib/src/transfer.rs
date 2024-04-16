use crate::net::NetworkPeer;
use crate::pairing::PairInfo;
use crate::protocol::{FileData, ResponseCode};

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use directories::UserDirs;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

async fn connect(a: SocketAddr) -> Result<(TcpStream, SocketAddr), std::io::Error> {
    log::info!("Trying connection to {} on port {}", a.ip(), a.port());

    Ok((TcpStream::connect(a).await?, a))
}

pub async fn send_file(file: &Path, pair_info: &mut PairInfo) {
    let mut options = pair_info
        .addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, a| {
            set.spawn(connect(a));
            set
        });

    let peer = loop {
        match options.join_next().await {
            Some(Ok(Ok((peer, address)))) => {
                log::info!("Connected to {}", address);
                break peer;
            }
            Some(_) => continue,
            None => {
                break pair_info
                    .fallback()
                    .expect("Connection to server must exist");
            }
        }
    };
    // NOTE: Drop fallback and futures if unused
    drop(pair_info.fallback());
    drop(options);

    let mut peer = NetworkPeer::new(Some(pair_info.cipher()), peer);

    // TODO: Buffer file
    let mut file = std::fs::read(file).unwrap();

    let hash = blake3::hash(&file);

    let file_data = FileData {
        hash: *hash.as_bytes(),
        file_size: file.len().to_be_bytes(),
        file_name: PathBuf::new(),
        file: Vec::new(),
    };

    peer.write_ser_enc(&file_data).await.unwrap();
    peer.write_enc(&mut file).await.unwrap();

    let response = peer.read_ser_enc::<ResponseCode>().await.unwrap();

    // TODO: Real response
    assert_eq!(response, ResponseCode::Ok, "Error sending file");
}

async fn bind(
    address: SocketAddr,
    bind_address: SocketAddr,
) -> Result<(TcpStream, SocketAddr), std::io::Error> {
    log::info!("Trying bind to {} on port {}", address.ip(), address.port());

    let listener = TcpListener::bind(bind_address).await?;

    let (peer, _) = listener.accept().await?;

    Ok((peer, address))
}

pub async fn receive_file(dest: Option<PathBuf>, pair_info: &mut PairInfo) {
    let dest = dest.unwrap_or_else(|| {
        UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .expect("Valid Download Directory")
    });

    let mut options =
        pair_info
            .bind_addresses()
            .into_iter()
            .fold(JoinSet::new(), |mut set, (a, b)| {
                set.spawn(bind(a, b));
                set
            });

    let peer = loop {
        match options.join_next().await {
            Some(Ok(Ok((peer, address)))) => {
                log::info!("Peer connected to {}", address);
                break peer;
            }
            Some(_) => continue,
            None => {
                break pair_info
                    .fallback()
                    .expect("Connection to server must exist");
            }
        }
    };
    // NOTE: Drop fallback and futures if unused
    drop(pair_info.fallback());
    drop(options);

    let mut peer = NetworkPeer::new(Some(pair_info.cipher()), peer);

    let file_data = peer.read_ser_enc::<FileData>().await.unwrap();
    // TODO: Use file name if exists

    // TODO: Check why normal vector does not work and fix
    let mut file = vec![0; usize::from_be_bytes(file_data.file_size)];

    peer.read_enc(&mut file).await.unwrap();

    peer.write_ser_enc(&ResponseCode::Ok).await.unwrap();

    // TODO: Handle Error
    assert_eq!(
        blake3::hash(&file),
        file_data.hash,
        "Error in file transfer, hashes are not the same"
    );

    std::fs::write(dest, file).expect("Can write file");
}
