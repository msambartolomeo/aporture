use std::ffi::OsString;
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

pub struct AportureTransferProtocol<'a, S: State> {
    pair_info: &'a mut PairInfo,
    path: &'a Path,
    _phantom: PhantomData<S>,
}

impl<'a> AportureTransferProtocol<'a, Sender> {
    pub fn new(pair_info: &'a mut PairInfo, path: &'a Path) -> Self {
        AportureTransferProtocol {
            pair_info,
            path,
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(self) -> Result<(), error::Send> {
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

        let tar_file = tokio::fs::File::from(tar_handle.await.expect("Task was aborted")?);

        let file_size = tar_file.metadata().await?.len();

        let file_data = FileData {
            file_size,
            // TODO: Test if this works cross platform (test also file_name.to_string_lossy())
            file_name,
            is_file,
        };

        peer.write_ser_enc(&file_data).await?;

        let hash = file::hash_and_send(tar_file, &mut peer).await?;

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
            _phantom: PhantomData,
        }
    }

    pub async fn transfer(self) -> Result<PathBuf, error::Receive> {
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

        let mut tar_file = tokio::fs::File::from(tempfile::tempfile()?);

        let hash = file::hash_and_receive(&mut tar_file, file_data.file_size, &mut peer).await?;

        let received_hash = peer.read_ser_enc::<Hash>().await?;

        let (response, result) = if hash == received_hash.0 {
            let mut tar_file = tar_file.into_std().await;

            let dest = tokio::task::spawn_blocking(move || {
                let mut suffix = 0;
                let extension = dest.extension().unwrap_or_default().to_owned();
                let file_name = dest.file_stem().expect("Pushed before").to_owned();

                while dest.try_exists().is_ok_and(|b| b) {
                    suffix += 1;

                    let extension = extension.clone();
                    let file_name = file_name.clone();

                    dest.set_file_name(
                        [file_name, extension].join(&OsString::from(format!("_{suffix}"))),
                    );
                }

                deflate::uncompress(&mut tar_file, dest, file_data.is_file)
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
