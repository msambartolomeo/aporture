use std::fs::{File, OpenOptions};
use std::io::Seek;
use std::path::{Path, PathBuf};

use flate2::Compression;

#[allow(unused)]
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

    let mut file = tar.into_inner()?.finish()?;

    // NOTE: Rewind so the file can be used again
    file.rewind()?;

    Ok(file)
}

#[allow(unused)]
pub fn uncompress(
    file: &mut File,
    mut dest: PathBuf,
    is_file: bool,
) -> Result<PathBuf, std::io::Error> {
    let dec = flate2::read::GzDecoder::new(file);

    let mut tar = tar::Archive::new(dec);

    if is_file {
        // NOTE: If the archive is to be treated as a file, it is assumed
        // that it only contains one element inside and we uncompress that.
        let mut entry = tar
            .entries()?
            .next()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))??;

        if let Err(e) = entry.unpack(&dest) {
            dest = crate::fs::downloads_directory().ok_or(e)?;

            entry.unpack_in(&dest)?;
        }
    } else {
        // NOTE: If it is a directory unpack the entire archive
        if let Err(e) = tar.unpack(&dest) {
            dest = crate::fs::downloads_directory().ok_or(e)?;

            tar.unpack(&dest)?;
        }
    };

    Ok(dest)
}
