use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use typed_path::Utf8PlatformPathBuf;

pub async fn non_existant(mut path: PathBuf) -> PathBuf {
    let mut suffix = 0;
    let extension = path.extension().map(OsStr::to_os_string);
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

pub fn sanitize(path: &Path) -> Result<PathBuf, std::io::Error> {
    let sanitized = if let Ok(sanitized) = std::fs::canonicalize(path) {
        if !sanitized.is_dir() && !sanitized.is_file() {
            return Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        }

        sanitized
    } else {
        let file_name = path
            .file_name()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;

        let parent = match path.parent() {
            Some(p) if p == PathBuf::default() => std::fs::canonicalize(PathBuf::from("."))?,
            Some(p) => std::fs::canonicalize(p)?,
            None => std::fs::canonicalize(path)?,
        };

        parent.join(file_name)
    };

    // NOTE: Test utf8 support
    let _ = sanitized
        .as_os_str()
        .to_str()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::Unsupported))?;

    Ok(sanitized)
}

pub fn platform(path: &Path) -> Utf8PlatformPathBuf {
    Utf8PlatformPathBuf::from(
        path.as_os_str()
            .to_str()
            .expect("Should be valid utf8 as path was sanitized"),
    )
}
