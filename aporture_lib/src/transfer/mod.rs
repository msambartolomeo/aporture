use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::NamedTempFile;
use tokio::task::JoinSet;
use walkdir::WalkDir;

use self::channel::{Channel, Message};
use crate::net::EncryptedNetworkPeer;
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{PairingResponseCode, TransferData, TransferResponseCode};
use crate::{Receiver, Sender, State};

mod channel;
mod deflate;
mod error;
mod file;
mod peer;

pub use channel::Message as ChannelMessage;
pub use error::{Receive as ReceiveError, Send as SendError};

pub struct AportureTransferProtocol<'a, S: State> {
    pair_info: &'a mut PairInfo,
    path: &'a Path,
    channel: Option<Channel>,
    _phantom: PhantomData<S>,
}

impl<'a, S: State> AportureTransferProtocol<'a, S> {
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
        let mut path = file::sanitize_path(self.path)
            .await
            .map_err(|_| error::Send::Path)?;

        log::info!("Sending file {}", path.display());

        let addresses = self.pair_info.addresses();
        let cipher = self.pair_info.cipher();

        let options_factory = || {
            addresses.iter().fold(JoinSet::new(), |mut set, a| {
                set.spawn(peer::connect(*a, Arc::clone(&cipher)));
                set
            })
        };

        let mut peer = peer::find(options_factory, self.pair_info).await;

        let mut transfer_data = get_transfer_data(&path)?;

        let tar_file_holder = if transfer_data.total_files > 150 {
            let file = compress_folder(&mut transfer_data, path, &mut peer, &self.channel).await?;
            path = file.path().to_owned();
            Some(file)
        } else {
            None
        };

        log::info!("Sending transfer data information {transfer_data:?}");
        peer.write_ser_enc(&transfer_data).await?;

        #[allow(clippy::cast_possible_truncation)]
        let progress_len = transfer_data.total_size as usize;
        channel::send(&self.channel, Message::ProgressSize(progress_len)).await;

        if path.is_file() {
            log::info!("Sending file...");

            file::send(&mut peer, &path, &path, &self.channel).await?;
        } else {
            for entry in WalkDir::new(&path).follow_links(true).into_iter().skip(1) {
                file::send(&mut peer, entry?.path(), &path, &self.channel).await?;
            }
        }

        // NOTE: keep the file alve until it is finished sending
        drop(tar_file_holder);

        channel::send(&self.channel, Message::Finished).await;
        if transfer_data.compressed {
            channel::send(&self.channel, Message::Uncompressing).await;
        }

        match peer.read_ser_enc::<TransferResponseCode>().await? {
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
        let addresses = self.pair_info.bind_addresses();
        let cipher = self.pair_info.cipher();

        let dest = file::sanitize_path(self.path)
            .await
            .map_err(|_| error::Receive::Destination)?;

        log::info!("File will try to be saved to {}", dest.display());

        let options_factory = || {
            addresses.iter().fold(JoinSet::new(), |mut set, (b, a)| {
                set.spawn(peer::bind(*b, *a, Arc::clone(&cipher)));
                set
            })
        };

        let mut peer = peer::find(options_factory, self.pair_info).await;

        log::info!("Receiving Transfer information");
        let mut transfer_data = peer.read_ser_enc::<TransferData>().await?;
        log::info!("Transfer data received: {transfer_data:?}");

        // NOTE: If the data should be compressed compressed, the sender will send the compressed information again
        if transfer_data.compressed {
            channel::send(&self.channel, Message::Compression).await;

            log::info!("Receiving tar.gz information");
            transfer_data = peer.read_ser_enc::<TransferData>().await?;
            log::info!("tar.gz received: {transfer_data:?}");
        }

        #[allow(clippy::cast_possible_truncation)]
        let progress_len = transfer_data.total_size as usize;
        channel::send(&self.channel, Message::ProgressSize(progress_len)).await;

        let dest = if transfer_data.total_files == 1 {
            receive_file(dest, &transfer_data, &mut peer, &self.channel).await?
        } else {
            receive_folder(dest, transfer_data, &mut peer, &self.channel).await?
        };

        peer.write_ser_enc(&PairingResponseCode::Ok).await?;

        Ok(dest)
    }
}

fn get_transfer_data(path: &Path) -> Result<TransferData, error::Send> {
    let mut transfer_data = WalkDir::new(&path)
        .follow_links(true)
        .into_iter()
        .try_fold(
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
        .expect("File Name Must be present as it was sanitized")
        .clone_into(&mut transfer_data.root_name);

    Ok(transfer_data)
}

async fn compress_folder(
    transfer_data: &mut TransferData,
    path: PathBuf,
    peer: &mut EncryptedNetworkPeer,
    channel: &Option<Channel>,
) -> Result<NamedTempFile, error::Send> {
    channel::send(channel, Message::Compression).await;
    transfer_data.compressed = true;

    log::info!("Sending original data information {transfer_data:?}");
    peer.write_ser_enc(&*transfer_data).await?;

    log::info!("Folder will be compressed as it is too big");

    let tar_file = tokio::task::spawn_blocking(move || deflate::compress(&path))
        .await
        .expect("Task was aborted")?;

    let metadata = tar_file.as_file().metadata()?;

    transfer_data.total_files = 1;
    transfer_data.total_size = metadata.len();

    // NOTE: Save the file so that it is not dropped and not deleted
    Ok(tar_file)
}

async fn receive_file(
    mut dest: PathBuf,
    transfer_data: &TransferData,
    peer: &mut EncryptedNetworkPeer,
    channel: &Option<Channel>,
) -> Result<PathBuf, error::Receive> {
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
        dest.push(&transfer_data.root_name);
    }

    let mut dest = file::non_existant_path(dest).await;

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

async fn receive_folder(
    mut dest: PathBuf,
    transfer_data: TransferData,
    peer: &mut EncryptedNetworkPeer,
    channel: &Option<Channel>,
) -> Result<PathBuf, error::Receive> {
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
        dest.push(transfer_data.root_name);
    }

    let dest = file::non_existant_path(dest).await;

    let tmp = dir.into_path();
    tokio::fs::rename(tmp, &dest).await?;

    Ok(dest)
}
