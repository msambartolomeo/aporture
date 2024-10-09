use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

use crate::crypto::hasher::{Hash, Hasher};
use crate::net::EncryptedNetworkPeer;
use crate::parser::EncryptedSerdeIO;

const BUFFER_SIZE: usize = 16 * 1024;

pub async fn hash_and_send(
    file: File,
    sender: &mut EncryptedNetworkPeer,
) -> Result<Hash, crate::io::Error> {
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

    Ok(hasher.finalize())
}

pub async fn hash_and_receive(
    file: &mut File,
    file_size: u64,
    receiver: &mut EncryptedNetworkPeer,
) -> Result<Hash, crate::io::Error> {
    let mut writer = BufWriter::new(file);
    let mut hasher = Hasher::default();
    let mut buffer = vec![0; BUFFER_SIZE];

    let file_size = usize::try_from(file_size).expect("u64 does not fit in usize");
    let mut read = 0;

    loop {
        let count = receiver.read_enc(&mut buffer).await?;

        read += count;

        if file_size == read {
            break;
        }

        assert!(read < file_size);

        if count == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset).into());
        }

        hasher.add(&buffer[..count]);
        writer.write_all(&buffer[..count]).await?;
    }

    Ok(hasher.finalize())
}
