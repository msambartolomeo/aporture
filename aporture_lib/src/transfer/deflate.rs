use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use flate2::Compression;

pub fn compress(path: &Path) -> Result<PathBuf, std::io::Error> {
    let tar_gz_path = path
        .with_extension("app")
        .with_extension("tar")
        .with_extension("gz");

    let tar_gz = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tar_gz_path)?;

    let enc = flate2::write::GzEncoder::new(tar_gz, Compression::default());

    let mut tar = tar::Builder::new(enc);

    if path.is_file() {
        tar.append_path(path)?;
    } else {
        tar.append_dir_all("", path)?;
    }

    tar.finish()?;

    Ok(tar_gz_path)
}

pub fn uncompress(path: &Path, dest: PathBuf, is_file: bool) -> Result<PathBuf, std::io::Error> {
    let tar_gz = OpenOptions::new().read(true).open(path)?;

    let dec = flate2::read::GzDecoder::new(tar_gz);

    let mut tar = tar::Archive::new(dec);

    if is_file {
        // NOTE: If the archive is to be treated as a file, it is assumed
        // that it only contains one element inside and we uncompress that.
        tar.entries()?
            .next()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))??
            .unpack(&dest)?;
    } else {
        // NOTE: If it is a directory unpack the entire archive
        tar.unpack(&dest)?;
    }

    Ok(dest)
}

pub fn compressed_path(path: &Path) -> PathBuf {
    path.with_extension("app.tar.gz")
}
