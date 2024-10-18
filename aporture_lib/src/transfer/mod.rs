use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::task::JoinSet;
use walkdir::WalkDir;

use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{PairingResponseCode, TransferData, TransferResponseCode};
use crate::{Receiver, Sender, State};

type Channel = tokio::sync::mpsc::Sender<ChannelMessage>;

pub enum ChannelMessage {
    Compression,
    ProgressSize(usize),
    Progress(usize),
    Uncompressing,
    Finished,
}

mod deflate;
mod file;
mod peer;

mod error;
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

        let file = if transfer_data.total_files > 150 {
            if let Some(ref channel) = self.channel {
                let _ = channel.send(ChannelMessage::Compression).await;
            }

            transfer_data.compressed = true;

            log::info!("Sending original data information {transfer_data:?}");
            peer.write_ser_enc(&transfer_data).await?;

            log::info!("Folder will be compressed as it is too big");

            let tar_file = tokio::task::spawn_blocking(move || deflate::compress(&path))
                .await
                .expect("Task was aborted")?;

            let metadata = tar_file.as_file().metadata()?;
            path = tar_file.path().to_path_buf();

            transfer_data.total_files = 1;
            transfer_data.total_size = metadata.len();

            // NOTE: Save the file so that it is not dropped and not deleted
            Some(tar_file)
        } else {
            None
        };

        log::info!("Sending transfer data information {transfer_data:?}");
        peer.write_ser_enc(&transfer_data).await?;

        if let Some(ref progress) = self.channel {
            #[allow(clippy::cast_possible_truncation)]
            let _ = progress
                .send(ChannelMessage::ProgressSize(
                    transfer_data.total_size as usize,
                ))
                .await;
        }

        if path.is_file() {
            log::info!("Sending file...");

            file::send(&mut peer, &path, &path, &self.channel).await?;
        } else {
            for entry in WalkDir::new(&path).follow_links(true).into_iter().skip(1) {
                file::send(&mut peer, entry?.path(), &path, &self.channel).await?;
            }
        }

        if let Some(ref channel) = self.channel {
            let _ = channel.send(ChannelMessage::Finished).await;
            if transfer_data.compressed {
                let _ = channel.send(ChannelMessage::Uncompressing).await;
            }
        }

        drop(file);

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

        let mut dest = file::sanitize_path(self.path)
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
            if let Some(ref channel) = self.channel {
                let _ = channel.send(ChannelMessage::Compression).await;
            }

            log::info!("Receiving tar.gz information");
            transfer_data = peer.read_ser_enc::<TransferData>().await?;
            log::info!("tar.gz received: {transfer_data:?}");
        }

        if let Some(ref channel) = self.channel {
            #[allow(clippy::cast_possible_truncation)]
            let _ = channel
                .send(ChannelMessage::ProgressSize(
                    transfer_data.total_size as usize,
                ))
                .await;
        }

        let dest = if transfer_data.total_files == 1 {
            let mut file = if dest.is_dir() {
                tempfile::NamedTempFile::new_in(&dest)?
            } else {
                let parent_path = dest
                    .parent()
                    .expect("Parent must exist as path is sanitized");

                tempfile::NamedTempFile::new_in(parent_path)?
            };

            file::receive(file.path(), &mut peer, &self.channel).await?;

            if let Some(ref channel) = self.channel {
                let _ = channel.send(ChannelMessage::Finished).await;
            }

            if dest.is_dir() {
                dest.push(&transfer_data.root_name);
            }

            let mut dest = file::non_existant_path(dest).await;

            if transfer_data.compressed {
                if let Some(ref channel) = self.channel {
                    let _ = channel.send(ChannelMessage::Uncompressing).await;
                }

                log::info!("Uncompressing file into path {}", dest.display());

                dest = tokio::task::spawn_blocking(move || {
                    deflate::uncompress(file.as_file_mut(), dest)
                })
                .await
                .expect("Task was aborted")?;
            } else {
                log::info!("Persisting file to path {}", dest.display());

                file.persist(&dest)
                    .map_err(|_| error::Receive::Destination)?;
            }

            dest
        } else {
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
                let file_data = file::receive(dir.path(), &mut peer, &self.channel).await?;

                if file_data.is_file {
                    files += 1;
                }
            }

            if let Some(ref channel) = self.channel {
                let _ = channel.send(ChannelMessage::Finished).await;
            }

            if dest.is_dir() {
                dest.push(transfer_data.root_name);
            }

            let dest = file::non_existant_path(dest).await;

            let tmp = dir.into_path();
            tokio::fs::rename(tmp, &dest).await?;

            dest
        };

        peer.write_ser_enc(&PairingResponseCode::Ok).await?;

        Ok(dest)
    }
}
