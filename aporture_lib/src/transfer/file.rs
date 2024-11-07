use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use tokio::fs::File;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

use crate::crypto;
use crate::crypto::hasher::Hasher;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, Hash, TransferResponseCode};
use crate::transfer::channel;
use crate::transfer::channel::{Channel, Message};

const FILE_RETRIES: usize = 3;
const BUFFER_SIZE: usize = 16 * 1024;

pub async fn send<Ep>(
    peer: &mut Ep,
    path: &Path,
    base: &Path,
    channel: &Option<Channel>,
) -> Result<(), super::error::Send>
where
    Ep: EncryptedSerdeIO + Send,
{
    let is_file = path.is_file();
    let file_size = if is_file { path.metadata()?.len() } else { 0 };

    // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
    let file_name = path
        .strip_prefix(base)
        .expect("Path must be a subpath from base")
        .to_owned()
        .into_os_string();

    log::info!("Sending file {}", path.display());

    let file_data = FileData {
        file_size,
        file_name,
        is_file,
    };

    peer.write_ser_enc(&file_data).await?;

    // NOTE: If it is a directory finish after sending name
    if !is_file {
        return Ok(());
    }

    for _ in 0..FILE_RETRIES {
        let file = OpenOptions::new().read(true).open(path).await?;

        let hash = hash_and_send(file, peer, channel).await?;

        peer.write_ser_enc(&Hash(hash)).await?;

        let response = peer.read_ser_enc::<TransferResponseCode>().await?;

        match response {
            TransferResponseCode::Ok => return Ok(()),
            TransferResponseCode::HashMismatch => {
                log::warn!(
                    "Hash mismatch in file transfer for {}, retrying...",
                    path.display()
                );
            }
        }
    }

    log::error!("Max retries reached for {}", path.display());

    Err(super::error::Send::HashMismatch)
}

pub async fn receive<Ep>(
    dest: &Path,
    peer: &mut Ep,
    channel: &Option<Channel>,
) -> Result<FileData, super::error::Receive>
where
    Ep: EncryptedSerdeIO + Send,
{
    let file_data = peer.read_ser_enc::<FileData>().await?;

    let mut path = dest.to_owned();

    let mut file = if dest.is_dir() {
        path.push(&file_data.file_name);

        if !file_data.is_file {
            tokio::fs::create_dir(path).await?;

            return Ok(file_data);
        }

        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?
    } else {
        OpenOptions::new().write(true).open(&path).await?
    };

    for _ in 0..FILE_RETRIES {
        log::info!("Receiving file {}", file_data.file_name.to_string_lossy());

        let hash = hash_and_receive(&mut file, file_data.file_size, peer, channel).await?;

        log::info!("File received");

        let received_hash = peer.read_ser_enc::<Hash>().await?;

        if hash == received_hash.0 {
            peer.write_ser_enc(&TransferResponseCode::Ok).await?;

            return Ok(file_data);
        }

        log::warn!("Calculated hash and received hash do not match, retrying...");

        peer.write_ser_enc(&TransferResponseCode::HashMismatch)
            .await?;
    }

    log::error!("Hash mismatch after retrying {FILE_RETRIES}");

    Err(super::error::Receive::HashMismatch)
}

async fn hash_and_send<Ep>(
    file: File,
    sender: &mut Ep,
    channel: &Option<Channel>,
) -> Result<crypto::hasher::Hash, crate::io::Error>
where
    Ep: EncryptedSerdeIO + Send,
{
    let mut reader = BufReader::new(file);
    let mut hasher = Hasher::default();
    let mut buffer = vec![0; BUFFER_SIZE];

    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }

        channel::send(channel, Message::Progress(count)).await;

        hasher.add(&buffer[..count]);
        sender.write_enc(&mut buffer[..count]).await?;
    }

    Ok(hasher.finalize())
}

async fn hash_and_receive<Ep>(
    file: &mut File,
    file_size: u64,
    receiver: &mut Ep,
    channel: &Option<Channel>,
) -> Result<crypto::hasher::Hash, crate::io::Error>
where
    Ep: EncryptedSerdeIO + Send,
{
    let mut writer = BufWriter::new(file);
    let mut hasher = Hasher::default();
    let mut buffer = vec![0; BUFFER_SIZE];

    let file_size = usize::try_from(file_size).expect("u64 does not fit in usize");
    let mut read = 0;

    while read < file_size {
        let count = receiver.read_enc(&mut buffer).await?;

        read += count;

        if count == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset).into());
        }

        channel::send(channel, Message::Progress(count)).await;

        hasher.add(&buffer[..count]);
        writer.write_all(&buffer[..count]).await?;
    }

    writer.flush().await?;

    Ok(hasher.finalize())
}

pub async fn non_existent_path(mut path: PathBuf) -> PathBuf {
    let mut suffix = 0;
    let extension = path.extension().map(std::ffi::OsStr::to_os_string);
    let file_name = path.file_stem().expect("Pushed before").to_owned();

    while tokio::fs::try_exists(&path).await.is_ok_and(|b| b) {
        suffix += 1;
        log::warn!(
            "Path {} is not valid, trying suffix {suffix}",
            path.display()
        );

        let mut file_name = file_name.clone();
        file_name.push(OsString::from(format!(" ({suffix})")));

        if let Some(ext) = extension.clone() {
            file_name.push(OsString::from("."));
            file_name.push(ext);
        }

        path.set_file_name(file_name);
    }

    path
}

pub async fn sanitize_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if let Ok(sanitized) = tokio::fs::canonicalize(&path).await {
        let metadata = tokio::fs::metadata(&sanitized).await?;

        if !metadata.is_dir() && !metadata.is_file() {
            return Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        }

        return Ok(sanitized);
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;

    let parent = match path.parent() {
        Some(p) if p == PathBuf::default() => tokio::fs::canonicalize(PathBuf::from(".")).await?,
        Some(p) => tokio::fs::canonicalize(p).await?,
        None => tokio::fs::canonicalize(path).await?,
    };

    let sanitized = parent.join(file_name);

    Ok(sanitized)
}
