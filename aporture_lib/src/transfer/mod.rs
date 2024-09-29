use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::task::JoinSet;

use crate::crypto::hasher::Hasher;
use crate::net::EncryptedNetworkPeer;
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, Hash, TransferResponseCode};

mod peer;

mod error;
pub use error::{Receive as ReceiveError, Send as SendError};

const BUFFER_SIZE: usize = 16 * 1024;

pub async fn send_file(path: &Path, pair_info: &mut PairInfo) -> Result<(), error::Send> {
    let Some(file_name) = path.file_name() else {
        return Err(error::Send::Path);
    };

    // TODO: Find better alternative
    // TODO: Retry on each future somehow
    // NOTE: Sleep to give time to receiver to bind
    tokio::time::sleep(Duration::from_secs(1)).await;

    let options = pair_info
        .addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, a| {
            set.spawn(peer::connect(a, pair_info.cipher()));
            set
        });

    let mut peer = peer::find(options, pair_info).await;

    // TODO: Buffer file
    let file = OpenOptions::new().read(true).open(path).await?;
    let file_data = FileData {
        file_size: file.metadata().await?.len(),
        // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
        file_name: file_name.to_owned(),
    };

    peer.write_ser_enc(&file_data).await?;

    let hash = hash_and_send(file, &mut peer).await?;

    peer.write_ser_enc(&hash).await?;

    let response = peer.read_ser_enc::<TransferResponseCode>().await?;

    match response {
        TransferResponseCode::Ok => {
            log::info!("File transferred correctly");
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
    let mut dest = dest
        .or_else(crate::fs::downloads_directory)
        .ok_or(ReceiveError::Directory)?;

    let options = pair_info
        .bind_addresses()
        .into_iter()
        .fold(JoinSet::new(), |mut set, (b, a)| {
            set.spawn(peer::bind(b, a, pair_info.cipher()));
            set
        });

    let mut peer = peer::find(options, pair_info).await;

    let file_data = peer.read_ser_enc::<FileData>().await?;

    if dest.is_dir() {
        dest.push(file_data.file_name);
    }

    let tmp_path = dest.with_extension("downloading").with_extension(".tmp");

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .await?;

    let hash = match hash_and_receive(file, &mut peer).await {
        Ok(hash) => hash,
        Err(e) => {
            tokio::fs::remove_file(tmp_path).await?;
            return Err(e.into());
        }
    };

    tokio::fs::rename(&tmp_path, &dest).await?;

    let received_hash = peer.read_ser_enc::<Hash>().await?;

    let response = if hash == received_hash {
        TransferResponseCode::Ok
    } else {
        TransferResponseCode::HashMismatch
    };

    peer.write_ser_enc(&response).await?;

    Ok(dest)
}

async fn hash_and_send(
    file: File,
    sender: &mut EncryptedNetworkPeer,
) -> Result<Hash, crate::io::Error>
where
{
    let mut reader = BufReader::new(file);
    let mut hasher = Hasher::default();
    let mut buffer = vec![0; BUFFER_SIZE];

    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }

        hasher.add(&buffer[..count]);
        sender.write_enc(&mut buffer[..count]).await?;
    }

    Ok(Hash(hasher.finalize()))
}

async fn hash_and_receive(
    file: File,
    receiver: &mut EncryptedNetworkPeer,
) -> Result<Hash, crate::io::Error> {
    let mut writer = BufWriter::new(file);
    let mut hasher = Hasher::default();
    let mut buffer = vec![0; BUFFER_SIZE];

    loop {
        let count = receiver.read_enc(&mut buffer).await?;
        if count == 0 {
            break;
        }

        hasher.add(&buffer[..count]);
        writer.write_all(&buffer[..count]).await?;
    }

    Ok(Hash(hasher.finalize()))
}
