//! Unix sockets live in a private, owned directory. Never repair untrusted paths.

use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "Unsafe Yntra socket directory or endpoint")
}

fn check_directory(path: &Path, private: bool) -> io::Result<()> {
    if !path.is_absolute() { return Err(denied()); }
    let uid = unsafe { libc::geteuid() };
    for (index, ancestor) in path.ancestors().enumerate() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err(denied()); }
        if index == 0 && private {
            if metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 { return Err(denied()); }
        } else {
            let sticky_root = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
            if (metadata.uid() != uid && metadata.uid() != 0)
                || (metadata.mode() & 0o022 != 0 && !sticky_root) { return Err(denied()); }
        }
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    check_directory(path.parent().ok_or_else(denied)?, false)?;
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    check_directory(path, true)
}

pub fn socket_path(name: &str) -> io::Result<PathBuf> {
    let directory = if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        let directory = PathBuf::from(runtime);
        check_directory(&directory, true)?;
        directory
    } else {
        // /tmp may be a system symlink on macOS. Validate its resolved directory.
        let directory = std::fs::canonicalize("/tmp")?
            .join(format!("yntra-{}-run", unsafe { libc::geteuid() }));
        create_private_directory(&directory)?;
        directory
    };
    Ok(directory.join(name))
}

pub async fn bind(path: &Path) -> io::Result<tokio::net::UnixListener> {
    check_directory(path.parent().ok_or_else(denied)?, true)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
                return Err(denied());
            }
            match tokio::time::timeout(std::time::Duration::from_secs(1), tokio::net::UnixStream::connect(path)).await {
                Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => std::fs::remove_file(path)?,
                _ => return Err(io::Error::new(io::ErrorKind::AddrInUse, "Yntra socket is already in use")),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shared_and_symlink_directories() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        create_private_directory(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(create_private_directory(&directory).is_err());
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        assert!(create_private_directory(&link).is_err());
    }

    #[tokio::test]
    async fn refuses_files_and_live_sockets_but_reclaims_stale_sockets() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("private");
        create_private_directory(&directory).unwrap();
        let path = directory.join("session.sock");
        std::fs::write(&path, b"preserve").unwrap();
        assert!(bind(&path).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve");
        std::fs::remove_file(&path).unwrap();
        let listener = bind(&path).await.unwrap();
        assert!(bind(&path).await.is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        drop(listener);
        assert!(bind(&path).await.is_ok());
    }
}
