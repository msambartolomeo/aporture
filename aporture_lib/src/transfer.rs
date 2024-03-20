use crate::pairing::{PairInfo, ResponseCode, TransferType};

use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use aes_gcm_siv::aead::{Aead, KeyInit};
use aes_gcm_siv::Aes256GcmSiv;
use blake3::Hash;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes, DisplayFromStr};

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
struct FileData {
    #[serde_as(as = "DisplayFromStr")]
    hash: Hash,

    #[serde_as(as = "Bytes")]
    file: Vec<u8>,
}

pub fn send_file(file: &Path, pair_info: &PairInfo) {
    let file = fs::read(file).expect("File exists");

    let hash = blake3::hash(&file);

    let file = encrypt(&file, &pair_info.key);

    let file_data = FileData { hash, file };

    let buf = serde_bencode::to_bytes(&file_data).expect("Correct serde parse");
    let mut peer = match pair_info.transfer_info {
        TransferType::Address(address) => {
            log::info!("connecting to {} on port {}", address.ip(), address.port());
            TcpStream::connect(address).expect("Connect to server")
        }
        _ => {
            unreachable!("Incorrect transferType")
        }
    };

    peer.write_all(&buf).expect("Write all");

    let mut buf = [0u8; 1024];

    let read = peer.read(&mut buf).expect("Read buffer");

    assert_ne!(read, 0, "Closed from reciever");

    let response: ResponseCode =
        serde_bencode::from_bytes(&buf).expect("server responds correctly");

    if matches!(response, ResponseCode::Ok) {
    } else {
        panic!("Server error");
    }

    peer.shutdown(Shutdown::Both).expect("Shutdown works");
}

pub fn recieve_file(dest: Option<PathBuf>, pair_info: &PairInfo) {
    let dest = dest.unwrap_or_else(|| {
        UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .expect("Valid Download Directory")
    });

    let listener = match pair_info.transfer_info {
        TransferType::UPnP { local_port, .. } => {
            log::info!("binding to {} on port {}", "0.0.0.0", local_port);

            TcpListener::bind((IpAddr::from([0, 0, 0, 0]), local_port)).expect("bind correct")
        }
        _ => {
            unreachable!("Incorrect transferType")
        }
    };

    let (mut peer, _) = listener.accept().expect("accept");

    log::info!("Conection achieved");

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

    fs::write(dest, file).expect("Can write file");
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
