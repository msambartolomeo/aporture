use std::ffi::OsString;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs::OpenOptions;
use tokio::task::JoinSet;
use walkdir::WalkDir;

use crate::net::EncryptedNetworkPeer;
use crate::pairing::PairInfo;
use crate::parser::EncryptedSerdeIO;
use crate::protocol::{FileData, Hash, TransferData, TransferResponseCode};
use crate::{Receiver, Sender, State};

mod deflate;
mod file;
mod peer;

mod error;
pub use error::{Receive as ReceiveError, Send as SendError};

const FILE_RETRIES: usize = 3;

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
        let path = sanitize_path(&self.path)
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

        transfer_data.root_name = path
            .file_name()
            .expect("File Name Must be present as it was sanitized")
            .to_owned();

        log::info!("Sending transfer data information {transfer_data:?}");
        peer.write_ser_enc(&transfer_data).await?;

        if let Some(ref progress) = self.channel {
            #[allow(clippy::cast_possible_truncation)]
            let _ = progress.send(transfer_data.total_size as usize).await;
        }

        if path.is_file() {
            log::info!("Sending file...");

            send_file(&mut peer, &path, &path, &self.channel).await?;
        } else {
            for entry in WalkDir::new(&path).follow_links(true).into_iter().skip(1) {
                send_file(&mut peer, entry?.path(), &path, &self.channel).await?;
            }
        }

        Ok(())

        // NOTE: build archive
        // let tar_handle = tokio::task::spawn_blocking(move || deflate::compress(&path));

        // log::info!("Waiting for file to be compressed...");

        // // let tar_file = tokio::fs::File::from(tar_handle.await.expect("Task was aborted")?);
        // let tar_file: tokio::fs::File = todo!();

        // log::info!("Compression finished");

        // let file_size = tar_file.metadata().await?.len();

        // let file_data = FileData {
        //     file_size,
        //     // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
        //     file_name,
        //     is_file: path.is_file(),
        // };

        // log::info!("Sending file information {file_data:?}");
        // peer.write_ser_enc(&file_data).await?;

        // log::info!("Sending file...");
        // if let Some(ref progress) = self.channel {
        //     #[allow(clippy::cast_possible_truncation)]
        //     let _ = progress.send(file_size as usize).await;
        // }

        // let hash = file::hash_and_send(tar_file, &mut peer, &self.channel).await?;
        // log::info!("File Sent");

        // peer.write_ser_enc(&Hash(hash)).await?;

        // let response = peer.read_ser_enc::<TransferResponseCode>().await?;

        // match response {
        //     TransferResponseCode::Ok => {
        //         log::info!("File transferred correctly");
        //         Ok(())
        //     }
        //     TransferResponseCode::HashMismatch => {
        //         log::error!("Hash mismatch in file transfer");
        //         Err(error::Send::HashMismatch)
        //     }
        // }
    }
}

async fn send_file(
    peer: &mut EncryptedNetworkPeer,
    path: &Path,
    base: &Path,
    channel: &Option<Channel>,
) -> Result<(), error::Send> {
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

        let hash = file::hash_and_send(file, peer, channel).await?;

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

    Err(error::Send::HashMismatch)
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

        let mut dest = sanitize_path(&self.path)
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

        log::info!("Receiving file information");
        let transfer_data = peer.read_ser_enc::<TransferData>().await?;
        log::info!("File data received: {transfer_data:?}");

        if let Some(ref progress) = self.channel {
            #[allow(clippy::cast_possible_truncation)]
            let _ = progress.send(transfer_data.total_size as usize).await;
        }

        if transfer_data.total_files == 1 {
            let file = if dest.is_dir() {
                tempfile::NamedTempFile::new_in(&dest)?
            } else {
                let parent_path = dest
                    .parent()
                    .expect("Parent must exist as path is sanitized");

                tempfile::NamedTempFile::new_in(parent_path)?
            };

            receive_file(file.path(), &mut peer, &self.channel).await?;

            if dest.is_dir() {
                dest.push(&transfer_data.root_name);
            }

            let dest = non_existant_path(dest).await;

            log::info!("Persisting file to path {}", dest.display());

            file.persist(&dest)
                .map_err(|_| error::Receive::Destination)?;

            Ok(dest)
        } else {
            let parent = dest
                .parent()
                .expect("Parent must exist as path is sanitized");

            if dest.is_file() || !parent.exists() {
                return Err(error::Receive::Destination);
            }

            // let root_data = peer.read_ser_enc::<FileData>().await?;

            // log::info!("Received root data: {root_data:?}");

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
                let file_data = receive_file(dir.path(), &mut peer, &self.channel).await?;

                if file_data.is_file {
                    files += 1;
                }
            }

            if dest.is_dir() {
                dest.push(transfer_data.root_name);
            }

            let dest = non_existant_path(dest).await;

            let tmp = dir.into_path();
            tokio::fs::rename(tmp, &dest).await?;

            Ok(dest)
        }

        // log::info!("Saving to path {}", dest.display());

        // let mut tar_file = tokio::fs::File::from(tempfile::tempfile()?);

        // log::info!("Receiving file...");

        // if let Some(ref progress) = self.channel {
        //     #[allow(clippy::cast_possible_truncation)]
        //     let _ = progress.send(file_data.file_size as usize).await;
        // }

        // let hash =
        //     file::hash_and_receive(&mut tar_file, file_data.file_size, &mut peer, &self.channel)
        //         .await?;
        // log::info!("File received");

        // let received_hash = peer.read_ser_enc::<Hash>().await?;

        // if hash != received_hash.0 {
        //     log::error!("Calculated hash and received hash do not match");

        //     peer.write_ser_enc(&TransferResponseCode::HashMismatch)
        //         .await?;

        //     return Err(error::Receive::HashMismatch);
        // }

        // let mut tar_file = tar_file.into_std().await;
        // tar_file.rewind()?;

        // let dest = tokio::task::spawn_blocking(move || {
        //     let mut suffix = 0;
        //     let extension = dest.extension().map(std::ffi::OsStr::to_os_string);
        //     let file_name = dest.file_stem().expect("Pushed before").to_owned();

        //     while dest.try_exists().is_ok_and(|b| b) {
        //         suffix += 1;
        //         log::warn!(
        //             "Path {} is not valid, trying suffix {suffix}",
        //             dest.display()
        //         );

        //         let mut file_name = file_name.clone();

        //         file_name.push(OsString::from(format!(" ({suffix})")));
        //         if let Some(ext) = extension.clone() {
        //             file_name.push(OsString::from("."));
        //             file_name.push(ext);
        //         }

        //         dest.set_file_name(file_name);
        //     }

        //     log::info!("Uncompressing file into {}", dest.display());

        //     deflate::uncompress(&mut tar_file, dest, file_data.is_file)
        // })
        // .await
        // .expect("Task was not aborted")?;

        // peer.write_ser_enc(&TransferResponseCode::Ok).await?;

        // Ok(dest)
    }
}

async fn receive_file(
    dest: &Path,
    peer: &mut EncryptedNetworkPeer,
    channel: &Option<Channel>,
) -> Result<FileData, error::Receive> {
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

        let hash = file::hash_and_receive(&mut file, file_data.file_size, peer, channel).await?;

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

    Err(error::Receive::HashMismatch)
}

async fn non_existant_path(mut path: PathBuf) -> PathBuf {
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

async fn sanitize_path(path: &Path) -> Result<PathBuf, std::io::Error> {
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
