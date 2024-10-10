use std::ffi::OsString;
use std::io::Seek;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::task::JoinSet;

use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, Hash, TransferResponseCode};
use crate::{Receiver, Sender, State};

mod deflate;
mod file;
mod peer;

mod error;
pub use error::{Receive as ReceiveError, Send as SendError};

type Channel = tokio::sync::mpsc::Sender<usize>;

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
        let path = tokio::fs::canonicalize(self.path)
            .await
            .map_err(|_| error::Send::Path)?;
        let file_name = path.file_name().ok_or(error::Send::Path)?.to_owned();

        log::info!(
            "Sending file {} with name {}",
            path.display(),
            file_name.to_string_lossy()
        );

        if !path.is_file() && !path.is_dir() {
            return Err(error::Send::Path);
        }

        let is_file = path.is_file();

        // NOTE: build archive
        let tar_handle = tokio::task::spawn_blocking(move || deflate::compress(&path));

        let addresses = self.pair_info.addresses();
        let cipher = self.pair_info.cipher();

        let options_factory = || {
            addresses.iter().fold(JoinSet::new(), |mut set, a| {
                set.spawn(peer::connect(*a, Arc::clone(&cipher)));
                set
            })
        };

        let mut peer = peer::find(options_factory, self.pair_info).await;

        let tar_file = tokio::fs::File::from(tar_handle.await.expect("Task was aborted")?);

        let file_size = tar_file.metadata().await?.len();

        let file_data = FileData {
            file_size,
            // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
            file_name,
            is_file,
        };

        log::info!("Sending file information {file_data:?}");
        peer.write_ser_enc(&file_data).await?;

        let hash = file::hash_and_send(tar_file, &mut peer, self.channel).await?;

        log::info!("Sending file...");
        peer.write_ser_enc(&Hash(hash)).await?;
        log::info!("File Sent");

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

        let options_factory = || {
            addresses.iter().fold(JoinSet::new(), |mut set, (b, a)| {
                set.spawn(peer::bind(*b, *a, Arc::clone(&cipher)));
                set
            })
        };

        let mut peer = peer::find(options_factory, self.pair_info).await;

        log::info!("Receiving file information");
        let file_data = peer.read_ser_enc::<FileData>().await?;

        log::info!("File data received: {file_data:?}");
        let mut dest = self.path.to_path_buf();

        if dest.try_exists()? && dest.canonicalize()?.is_dir() {
            dest.push(file_data.file_name);
        }

        log::info!("Saving to path {}", dest.display());

        let mut tar_file = tokio::fs::File::from(tempfile::tempfile()?);

        log::info!("Receiving file...");
        let hash =
            file::hash_and_receive(&mut tar_file, file_data.file_size, &mut peer, self.channel)
                .await?;
        log::info!("File received");

        let received_hash = peer.read_ser_enc::<Hash>().await?;

        if hash != received_hash.0 {
            log::error!("Calculated hash and received hash do not match");

            peer.write_ser_enc(&TransferResponseCode::HashMismatch)
                .await?;

            return Err(error::Receive::HashMismatch);
        }

        let mut tar_file = tar_file.into_std().await;
        tar_file.rewind()?;

        let dest = tokio::task::spawn_blocking(move || {
            let mut suffix = 0;
            let extension = dest.extension().unwrap_or_default().to_owned();
            let file_name = dest.file_stem().expect("Pushed before").to_owned();

            while dest.try_exists().is_ok_and(|b| b) {
                suffix += 1;
                log::warn!(
                    "Path {} is not valid, trying suffix {suffix}",
                    dest.display()
                );

                let extension = extension.clone();
                let file_name = file_name.clone();

                dest.set_file_name(
                    [file_name, extension].join(&OsString::from(format!(" ({suffix})."))),
                );
            }

            log::info!("Uncompressing file into {}", dest.display());

            deflate::uncompress(&mut tar_file, dest, file_data.is_file)
        })
        .await
        .expect("Task was not aborted")?;

        peer.write_ser_enc(&TransferResponseCode::Ok).await?;

        Ok(dest)
    }
}
