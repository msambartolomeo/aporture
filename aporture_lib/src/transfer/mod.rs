use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs::OpenOptions;
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

pub struct AportureTransferProtocol<'a, S: State> {
    pair_info: &'a mut PairInfo,
    path: &'a Path,
    tar_file: Option<PathBuf>,
    _phantom: PhantomData<S>,
}

impl<'a> AportureTransferProtocol<'a, Sender> {
    pub fn new(pair_info: &'a mut PairInfo, path: &'a Path) -> Self {
        AportureTransferProtocol {
            pair_info,
            path,
            tar_file: None,
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(mut self) -> Result<(), error::Send> {
        let path = tokio::fs::canonicalize(self.path)
            .await
            .map_err(|_| error::Send::Path)?;
        let file_name = path.file_name().ok_or(error::Send::Path)?.to_owned();

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

        // NOTE: Add to struct for it to be deleted on Drop
        self.tar_file = Some(tar_handle.await.expect("Task was not aborted")?);

        let tar_path = self
            .tar_file
            .as_ref()
            .expect("Exists as it was added just before");

        let file = OpenOptions::new().read(true).open(tar_path).await?;

        let file_data = FileData {
            file_size: file.metadata().await?.len(),
            is_file,
            // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
            file_name,
        };

        peer.write_ser_enc(&file_data).await?;

        let hash = file::hash_and_send(file, &mut peer).await?;

        peer.write_ser_enc(&Hash(hash)).await?;

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
            tar_file: None,
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(mut self) -> Result<PathBuf, error::Receive> {
        let mut dest = tokio::fs::canonicalize(self.path).await?;

        let addresses = self.pair_info.bind_addresses();
        let cipher = self.pair_info.cipher();

        let options_factory = || {
            addresses.iter().fold(JoinSet::new(), |mut set, (b, a)| {
                set.spawn(peer::bind(*b, *a, Arc::clone(&cipher)));
                set
            })
        };

        let mut peer = peer::find(options_factory, self.pair_info).await;

        let file_data = peer.read_ser_enc::<FileData>().await?;

        if dest.is_dir() {
            dest.push(file_data.file_name);
        }

        let tar_path = deflate::compressed_path(&dest);
        self.tar_file = Some(tar_path.clone());

        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tar_path)
            .await?;

        let hash = file::hash_and_receive(file, file_data.file_size, &mut peer).await?;

        let received_hash = peer.read_ser_enc::<Hash>().await?;

        let (response, result) = if hash == received_hash.0 {
            let dest = tokio::task::spawn_blocking(move || {
                deflate::uncompress(&tar_path, dest, file_data.is_file)
            })
            .await
            .expect("Task was not aborted")?;
            (TransferResponseCode::Ok, Ok(dest))
        } else {
            (
                TransferResponseCode::HashMismatch,
                Err(error::Receive::HashMismatch),
            )
        };

        peer.write_ser_enc(&response).await?;

        result
    }
}

impl<'a, S: State> Drop for AportureTransferProtocol<'a, S> {
    fn drop(&mut self) {
        if let Some(ref path) = self.tar_file {
            let _ = std::fs::remove_file(path);
        }
    }
}
