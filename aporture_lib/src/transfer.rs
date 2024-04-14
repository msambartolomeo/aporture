use crate::crypto::Cipher;
use crate::net::NetworkPeer;
use crate::pairing::{PairInfo, TransferInfo};
use crate::protocol::{FileData, ResponseCode};

use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};

use aes_gcm_siv::aead::{Aead, KeyInit};
use aes_gcm_siv::Aes256GcmSiv;
use directories::UserDirs;
use tokio::net::TcpStream;
use tokio::task::JoinSet;

async fn connect(a: SocketAddr) -> Result<(TcpStream, SocketAddr), std::io::Error> {
    log::info!("Connection to {} on port {}", a.ip(), a.port());

    Ok((TcpStream::connect(a).await?, a))
}

pub async fn send_file(file: &Path, cipher: Cipher, addresses: &[SocketAddr]) {
    let mut options = addresses.iter().fold(JoinSet::new(), |mut set, &a| {
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
                // TODO: Handle error
                return;
            }
        }
    };
    drop(options);

    let mut peer = NetworkPeer::new(Some(cipher), peer);

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

    let response = peer.read_ser::<ResponseCode>().await.unwrap();

    assert_eq!(response, ResponseCode::Ok, "Error sending file");
}

pub fn receive_file(dest: Option<PathBuf>, pair_info: &PairInfo) {
    let dest = dest.unwrap_or_else(|| {
        UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .expect("Valid Download Directory")
    });

    let listener = match pair_info.transfer_info {
        TransferInfo::UPnP { local_port, .. } => {
            log::info!("binding to {} on port {}", "0.0.0.0", local_port);

            TcpListener::bind((IpAddr::from([0, 0, 0, 0]), local_port)).expect("bind correct")
        }
        _ => {
            unreachable!("Incorrect transferType")
        }
    };

    let (mut peer, _) = listener.accept().expect("accept");

    log::info!("Connection achieved");

    let mut buf = [0u8; 1024];

    let read = peer.read(&mut buf).expect("Read buffer");

    assert_ne!(read, 0, "Closed from sender");

    let file_data: FileData = serde_bencode::from_bytes(&buf).expect("serde works");

    let buf = serde_bencode::to_bytes(&ResponseCode::Ok).expect("Translation to bencode");

    peer.write_all(&buf).expect("Write all");

    // // TODO: Check why normal vector does not work and fix
    // let mut file = [0u8; 4096];

    // let read = peer.read(&mut file).expect("Read buffer");

    // assert_ne!(read, 0, "Closed from sender");

    peer.shutdown(Shutdown::Both).expect("Shutdown works");

    let file = decrypt(&file_data.file, &pair_info.key);

    assert_eq!(
        blake3::hash(&file),
        file_data.hash,
        "Error in file transfer, hashes are not the same"
    );

    std::fs::write(dest, file).expect("Can write file");
}

#[must_use]
pub fn encrypt(plain: &[u8], key: &[u8]) -> Vec<u8> {
    let key = key.into();
    // TODO: Get a real nonce
    let nonce = b"unique nonce".into();
    let aes = Aes256GcmSiv::new(key);

    aes.encrypt(nonce, plain).expect("Encryption failure")
}

#[must_use]
pub fn decrypt(cipher: &[u8], key: &[u8]) -> Vec<u8> {
    let key = key.into();
    // TODO: Get a real nonce
    let nonce = b"unique nonce".into();
    let aes = Aes256GcmSiv::new(key);

    aes.decrypt(nonce, cipher).expect("Decryption failure")
}
