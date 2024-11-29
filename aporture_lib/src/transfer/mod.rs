use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use typed_path::Utf8NativePathBuf;
use walkdir::WalkDir;

use self::channel::{Channel, Message};
use crate::net::peer::{Encryptable, Peer};
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{PairingResponseCode, TransferData, TransferResponseCode};
use crate::{Receiver, Sender, State};

mod channel;
mod connection;
mod deflate;
mod error;
mod file;

pub use channel::Message as ChannelMessage;
pub use error::{Receive as ReceiveError, Send as SendError};

pub struct AportureTransferProtocol<'a, S: State> {
    pair_info: &'a mut PairInfo,
    path: &'a Path,
    channel: Option<Channel>,
    _phantom: PhantomData<S>,
}

impl<S: State> AportureTransferProtocol<'_, S> {
    pub fn add_progress_notifier(&mut self, channel: Channel) {
        self.channel = Some(channel);
    }
}

impl<'a> AportureTransferProtocol<'a, Sender> {
    pub fn new(pair_info: &'a mut PairInfo, path: &'a Path) -> Self {
        AportureTransferProtocol {
            pair_info,
            path,
            channel: None,
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(self) -> Result<(), error::Send> {
        let connection = connection::find(self.pair_info).await;

        if let Some(connection) = connection {
            let peer = connection.new_stream().await?;

            self.transfer_peer(peer).await?;

            connection.finish().await;
        } else {
            log::info!("Timeout waiting for peer connection, using server fallback");
            let peer = self
                .pair_info
                .fallback()
                .expect("Connection to server must exist")
                .add_cipher(self.pair_info.cipher());

            self.transfer_peer(peer).await?;
        }

        Ok(())
    }

    async fn transfer_peer<Ep>(self, mut peer: Ep) -> Result<(), error::Send>
    where
        Ep: Encryptable + Peer + Send,
    {
        let mut path = file::sanitize_path(self.path).map_err(|_| error::Send::Path)?;

        log::info!("Sending file {}", path.display());
        let mut transfer_data = get_transfer_data(&path)?;

        let tar_file_holder = if transfer_data.total_files > 150 {
            let file = compress_folder(&mut transfer_data, &path, &mut peer, self.channel.as_ref())
                .await?;
            file.path().clone_into(&mut path);
            Some(file)
        } else {
            None
        };

        log::info!("Sending transfer data information {transfer_data:?}");
        peer.write_ser_enc(&transfer_data).await?;

        #[allow(clippy::cast_possible_truncation)]
        let progress_len = transfer_data.total_size as usize;
        channel::send(self.channel.as_ref(), Message::ProgressSize(progress_len)).await;

        let base = Utf8NativePathBuf::from(
            path.as_mut_os_str()
                .to_str()
                .expect("Should be valid utf8 as path was sanitized"),
        );

        if path.is_file() {
            log::info!("Sending file...");

            file::send(&mut peer, &path, &base, self.channel.as_ref()).await?;
        } else {
            for entry in WalkDir::new(&path).follow_links(true).into_iter().skip(1) {
                file::send(&mut peer, entry?.path(), &base, self.channel.as_ref()).await?;
            }
        }

        // NOTE: keep the file alive until it is finished sending
        drop(tar_file_holder);

        channel::send(self.channel.as_ref(), Message::Finished).await;
        if transfer_data.compressed {
            channel::send(self.channel.as_ref(), Message::Uncompressing).await;
        }

        let response = peer.read_ser_enc::<TransferResponseCode>().await?;

        match response {
            TransferResponseCode::Ok => Ok(()),
            TransferResponseCode::HashMismatch => Err(error::Send::HashMismatch),
        }
    }
}

impl<'a> AportureTransferProtocol<'a, Receiver> {
    pub fn new(pair_info: &'a mut PairInfo, dest: &'a Path) -> Self {
        AportureTransferProtocol {
            pair_info,
            path: dest,
            channel: None,
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(self) -> Result<PathBuf, error::Receive> {
        let connection = connection::find(self.pair_info).await;

        let destination = if let Some(connection) = connection {
            let peer = connection.new_stream().await?;

            let destination = self.transfer_peer(peer).await?;

            connection.finish().await;

            destination
        } else {
            log::info!("Timeout waiting for peer connection, using server fallback");
            let peer = self
                .pair_info
                .fallback()
                .expect("Connection to server must exist")
                .add_cipher(self.pair_info.cipher());

            self.transfer_peer(peer).await?
        };

        Ok(destination)
    }

    async fn transfer_peer<Ep>(self, mut peer: Ep) -> Result<PathBuf, error::Receive>
    where
        Ep: Encryptable + Peer + Send,
    {
        let dest = file::sanitize_path(self.path).map_err(|_| error::Receive::Destination)?;

        log::info!("File will try to be saved to {}", dest.display());

        log::info!("Receiving Transfer information");
        let mut transfer_data = peer.read_ser_enc::<TransferData>().await?;
        log::info!("Transfer data received: {transfer_data:?}");

        // NOTE: If the data should be compressed, the sender will send the compressed information again
        if transfer_data.compressed {
            channel::send(self.channel.as_ref(), Message::Compression).await;

            log::info!("Receiving tar.gz information");
            transfer_data = peer.read_ser_enc::<TransferData>().await?;
            log::info!("tar.gz received: {transfer_data:?}");
        }

        #[allow(clippy::cast_possible_truncation)]
        let progress_len = transfer_data.total_size as usize;
        channel::send(self.channel.as_ref(), Message::ProgressSize(progress_len)).await;

        let dest = if transfer_data.total_files == 1 {
            receive_file(dest, &transfer_data, &mut peer, self.channel.as_ref()).await?
        } else {
            receive_folder(dest, transfer_data, &mut peer, self.channel.as_ref()).await?
        };

        peer.write_ser_enc(&PairingResponseCode::Ok).await?;

        Ok(dest)
    }
}

fn get_transfer_data(path: &Path) -> Result<TransferData, error::Send> {
    let mut transfer_data = WalkDir::new(path).follow_links(true).into_iter().try_fold(
        TransferData::default(),
        |mut data, entry| -> Result<TransferData, error::Send> {
            let metadata = entry?.metadata()?;

            if metadata.is_file() {
                let file_length = metadata.len();

                data.total_files += 1;
                data.total_size += file_length;
            }
            Ok(data)
        },
    )?;

    path.file_name()
        .expect("Should have a File Name as path it was sanitized")
        .to_str()
        .expect("Should be valid utf8 as it was santized")
        .clone_into(&mut transfer_data.root_name);

    Ok(transfer_data)
}

async fn compress_folder<Ep>(
    transfer_data: &mut TransferData,
    path: &Path,
    peer: &mut Ep,
    channel: Option<&Channel>,
) -> Result<NamedTempFile, error::Send>
where
    Ep: EncryptedSerdeIO + Send,
{
    channel::send(channel, Message::Compression).await;
    transfer_data.compressed = true;

    log::info!("Sending original data information {transfer_data:?}");
    peer.write_ser_enc(&*transfer_data).await?;

    log::info!("Folder will be compressed as it is too big");

    let p = path.to_owned();
    let tar_file = tokio::task::spawn_blocking(move || deflate::compress(&p))
        .await
        .expect("Task was aborted")?;

    let metadata = tar_file.as_file().metadata()?;

    transfer_data.total_files = 1;
    transfer_data.total_size = metadata.len();

    Ok(tar_file)
}

async fn receive_file<Ep>(
    mut dest: PathBuf,
    transfer_data: &TransferData,
    peer: &mut Ep,
    channel: Option<&Channel>,
) -> Result<PathBuf, error::Receive>
where
    Ep: EncryptedSerdeIO + Send,
{
    let mut file = if dest.is_dir() {
        tempfile::NamedTempFile::new_in(&dest)?
    } else {
        let parent_path = dest
            .parent()
            .expect("Parent must exist as path is sanitized");

        tempfile::NamedTempFile::new_in(parent_path)?
    };

    file::receive(file.path(), peer, channel).await?;

    channel::send(channel, Message::Finished).await;

    if dest.is_dir() {
        let path = Utf8NativePathBuf::from(&transfer_data.root_name);

        dest.push(path.normalize());
    }

    let mut dest = file::non_existent_path(dest).await;

    if transfer_data.compressed {
        channel::send(channel, Message::Uncompressing).await;
        log::info!("Uncompressing file into path {}", dest.display());

        dest = tokio::task::spawn_blocking(move || deflate::uncompress(file.as_file_mut(), dest))
            .await
            .expect("Task was aborted")?;
    } else {
        log::info!("Persisting file to path {}", dest.display());

        file.persist(&dest)
            .map_err(|_| error::Receive::Destination)?;
    }

    Ok(dest)
}

async fn receive_folder<Ep>(
    mut dest: PathBuf,
    transfer_data: TransferData,
    peer: &mut Ep,
    channel: Option<&Channel>,
) -> Result<PathBuf, error::Receive>
where
    Ep: EncryptedSerdeIO + Send,
{
    let parent = dest
        .parent()
        .expect("Parent must exist as path is sanitized");

    if dest.is_file() || !parent.exists() {
        return Err(error::Receive::Destination);
    }

    let base_path = if tokio::fs::try_exists(&dest)
        .await
        .map_err(|_| error::Receive::Destination)?
    {
        &dest
    } else {
        parent
    };

    let dir = tempfile::tempdir_in(base_path)?;

    let mut files = 0;

    while files < transfer_data.total_files {
        let file_data = file::receive(dir.path(), peer, channel).await?;

        if file_data.is_file {
            files += 1;
        }
    }

    channel::send(channel, Message::Finished).await;

    if dest.is_dir() {
        let path = Utf8NativePathBuf::from(&transfer_data.root_name);

        dest.push(path.normalize());
    }

    let dest = file::non_existent_path(dest).await;

    let tmp = dir.into_path();
    tokio::fs::rename(tmp, &dest).await?;

    Ok(dest)
}
