use std::path::Path;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use typed_path::{Utf8PlatformPath, Utf8UnixPathBuf};

use crate::crypto;
use crate::crypto::hasher::Hasher;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, Hash};
use crate::transfer::channel::{Channel, Message};
use crate::transfer::{channel, path};

const BUFFER_SIZE: usize = 16 * 1024;

pub async fn send<Ep>(
    peer: &mut Ep,
    id: usize,
    path: &Path,
    base: &Utf8PlatformPath,
    channel: Option<&Channel>,
) -> Result<(), super::error::Send>
where
    Ep: EncryptedSerdeIO + Send,
{
    let is_file = path.is_file();
    let file_size = if is_file { path.metadata()?.len() } else { 0 };

    let path = path::platform(path);

    let file_name = path
        .strip_prefix(base)
        .expect("Path must be a subpath from base")
        .with_unix_encoding()
        .to_string();

    log::info!("Sending file {}", path);

    let file_data = FileData {
        id: id as u64,
        file_size,
        file_name,
        is_file,
    };

    peer.write_ser_enc(&file_data).await?;

    // NOTE: If it is a directory finish after sending name
    if !is_file {
        return Ok(());
    }

    let file = OpenOptions::new().read(true).open(&path).await?;

    let hash = hash_and_send(file, peer, channel).await?;

    peer.write_ser_enc(&Hash(hash)).await?;

    Ok(())
}

pub async fn receive<Ep>(
    dest: &Path,
    peer: &mut Ep,
    channel: Option<&Channel>,
) -> Result<(FileData, bool), super::error::Receive>
where
    Ep: EncryptedSerdeIO + Send,
{
    let file_data = peer.read_ser_enc::<FileData>().await?;
    let received_path = Utf8UnixPathBuf::from(&file_data.file_name)
        .normalize()
        .with_platform_encoding();
    let mut path = path::platform(dest);

    let file = if dest.is_dir() {
        path.push(&received_path);

        if !file_data.is_file {
            tokio::fs::create_dir(&path).await?;

            return Ok((file_data, false));
        }

        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?
    } else {
        OpenOptions::new().write(true).open(&path).await?
    };

    log::info!("Receiving file {}", &received_path);

    let hash = hash_and_receive(file, file_data.file_size, peer, channel).await?;

    log::info!("File received");

    let received_hash = peer.read_ser_enc::<Hash>().await?;

    if hash != received_hash.0 {
        log::warn!(
            "Calculated hash and received hash do not match for file {}, id {}",
            file_data.file_name,
            file_data.id,
        );
    }

    Ok((file_data, hash != received_hash.0))
}

async fn hash_and_send<Ep>(
    file: File,
    sender: &mut Ep,
    channel: Option<&Channel>,
) -> Result<crypto::hasher::Hash, crate::io::Error>
where
    Ep: EncryptedSerdeIO + Send,
{
    let mut reader = BufReader::with_capacity(10 * BUFFER_SIZE, file);
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
    file: File,
    file_size: u64,
    receiver: &mut Ep,
    channel: Option<&Channel>,
) -> Result<crypto::hasher::Hash, crate::io::Error>
where
    Ep: EncryptedSerdeIO + Send,
{
    let mut writer = BufWriter::with_capacity(10 * BUFFER_SIZE, file);
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
