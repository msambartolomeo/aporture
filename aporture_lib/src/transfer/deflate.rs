use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use flate2::Compression;

pub fn compress(path: &Path) -> Result<File, std::io::Error> {
    let file = tempfile::tempfile()?;

    let enc = flate2::write::GzEncoder::new(file, Compression::default());

    let mut tar = tar::Builder::new(enc);

    if path.is_file() {
        let mut file = OpenOptions::new().read(true).open(path)?;
        tar.append_file(
            path.file_name()
                .expect("File is absolute and requires filename"),
            &mut file,
        )?;
    } else {
        tar.append_dir_all("", path)?;
    }

    let file = tar.into_inner()?.finish()?;

    Ok(file)
}

pub fn uncompress(
    file: &mut File,
    dest: PathBuf,
    is_file: bool,
) -> Result<PathBuf, std::io::Error> {
    let dec = flate2::read::GzDecoder::new(file);

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
