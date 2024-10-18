use std::fs::File;
use std::path::{Path, PathBuf};

use flate2::Compression;
use tempfile::NamedTempFile;

#[allow(unused)]
pub fn compress(path: &Path) -> Result<NamedTempFile, std::io::Error> {
    let file = tempfile::NamedTempFile::new()?;

    let enc = flate2::write::GzEncoder::new(file, Compression::default());

    let mut tar = tar::Builder::new(enc);

    tar.append_dir_all("", path)?;

    let mut file = tar.into_inner()?.finish()?;

    Ok(file)
}

#[allow(unused)]
pub fn uncompress(file: &mut File, dest: PathBuf) -> Result<PathBuf, std::io::Error> {
    let dec = flate2::read::GzDecoder::new(file);

    let mut tar = tar::Archive::new(dec);

    tar.unpack(&dest)?;

    Ok(dest)
}
