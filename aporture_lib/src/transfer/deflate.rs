use std::fs::File;
use std::path::{Path, PathBuf};

use flate2::Compression;
use tempfile::NamedTempFile;

use crate::parser::EncryptedSerdeIO;
use crate::protocol::TransferData;
use crate::transfer::channel::{self, Channel, Message};

#[allow(unused)]
async fn compress_folder<Ep>(
    transfer_data: &mut TransferData,
    path: &Path,
    peer: &mut Ep,
    channel: Option<&Channel>,
) -> Result<NamedTempFile, super::error::Send>
where
    Ep: EncryptedSerdeIO + Send,
{
    channel::send(channel, Message::Compression).await;
    transfer_data.compressed = true;

    log::info!("Sending original data information {transfer_data:?}");
    peer.write_ser_enc(&*transfer_data).await?;

    log::info!("Folder will be compressed as it is too big");

    let p = path.to_owned();
    let tar_file = tokio::task::spawn_blocking(move || compress(&p))
        .await
        .expect("Task was aborted")?;

    let metadata = tar_file.as_file().metadata()?;

    transfer_data.total_files = 1;
    transfer_data.total_size = metadata.len();

    Ok(tar_file)
}

#[allow(unused)]
pub fn compress(path: &Path) -> Result<NamedTempFile, std::io::Error> {
    let file = tempfile::NamedTempFile::new()?;

    let enc = flate2::write::GzEncoder::new(file, Compression::default());

    let mut tar = tar::Builder::new(enc);

    tar.append_dir_all("", path)?;

    let file = tar.into_inner()?.finish()?;

    Ok(file)
}

#[allow(unused)]
pub fn uncompress(file: &mut File, dest: PathBuf) -> Result<PathBuf, std::io::Error> {
    let dec = flate2::read::GzDecoder::new(file);

    let mut tar = tar::Archive::new(dec);

    tar.unpack(&dest)?;

    Ok(dest)
}
