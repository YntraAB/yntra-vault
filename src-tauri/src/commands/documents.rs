//! Android's document picker returns content URIs, not filesystem paths.
use std::io::{Read, Write};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
use zeroize::Zeroizing;

fn file_path(path: &str) -> Result<FilePath, String> {
    if path.starts_with("content://") {
        if !cfg!(target_os = "android") {
            return Err("Android document URI on another platform".into());
        }
        Ok(FilePath::Url(
            path.parse().map_err(|_| "Invalid document URI")?,
        ))
    } else {
        Ok(FilePath::Path(path.into()))
    }
}

pub fn read(app: &tauri::AppHandle, path: &str, limit: u64) -> Result<Zeroizing<Vec<u8>>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    let file = app
        .fs()
        .open(file_path(path)?, options)
        .map_err(|e| e.to_string())?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Document exceeds the allowed size".into());
    }
    Ok(bytes)
}

pub fn read_keyfile(
    app: &tauri::AppHandle,
    path: &str,
) -> Result<yntra_vault_core::crypto::LockedBuffer, String> {
    if !path.starts_with("content://") {
        return yntra_vault_core::vault::manager::read_key_file_safely(std::path::Path::new(path))
            .map_err(|e| e.to_string());
    }
    let bytes = read(app, path, 32 * 1024 * 1024)?;
    if bytes.is_empty() {
        return Err("Key file is empty".into());
    }
    Ok(yntra_vault_core::crypto::LockedBuffer::new(&bytes))
}

pub fn write(app: &tauri::AppHandle, path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = app
        .fs()
        .open(file_path(path)?, options)
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    if !path.starts_with("content://") {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .map_err(|e| e.to_string())?;
    // Document providers may return pipes rather than regular files.
    if !path.starts_with("content://") {
        file.sync_all().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn protect_vault_destination(vault: &std::path::Path, destination: &str) -> Result<(), String> {
    if destination.starts_with("content://") {
        return Ok(());
    }
    let target = std::path::Path::new(destination);
    if target == vault
        || matches!((target.canonicalize(), vault.canonicalize()), (Ok(a), Ok(b)) if a == b)
    {
        return Err(
            "Choose a different file: exporting here would overwrite the active vault".into(),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn import_vault_document(app: tauri::AppHandle, path: String) -> Result<String, String> {
    if !path.starts_with("content://") {
        return Ok(path);
    }
    let bytes = read(&app, &path, 512 * 1024 * 1024)?;
    if !(bytes.starts_with(b"YNS2") || bytes.starts_with(b"YNTR")) {
        return Err("Not a supported vault file".into());
    }
    let directory = super::auth::vault_storage_dir(&app)?;
    let destination = directory.join(format!("imported-{}.vdb", uuid::Uuid::new_v4()));
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temporary
        .persist_noclobber(&destination)
        .map_err(|e| e.to_string())?;
    Ok(destination.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn export_attachment(
    app: tauri::AppHandle,
    entry_id: String,
    attachment_id: String,
    path: String,
    state: tauri::State<'_, super::AppState>,
) -> Result<(), String> {
    let bytes = {
        let vault = state.vault.lock().map_err(|e| e.to_string())?;
        let manager = vault.as_ref().ok_or("Vault is locked")?;
        protect_vault_destination(&manager.path, &path)?;
        Zeroizing::new(
            manager
                .get_attachment_data(
                    entry_id.parse().map_err(|_| "Invalid entry ID")?,
                    attachment_id.parse().map_err(|_| "Invalid attachment ID")?,
                )
                .map_err(|e| e.to_string())?,
        )
    };
    write(&app, &path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_cannot_destroy_the_active_vault() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("vault.vdb");
        std::fs::write(&path, b"synthetic encrypted vault").unwrap();
        assert!(protect_vault_destination(&path, &path.to_string_lossy()).is_err());
        let alias = directory.path().join(".").join("vault.vdb");
        assert!(protect_vault_destination(&path, &alias.to_string_lossy()).is_err());
        assert!(
            protect_vault_destination(
                &path,
                &directory.path().join("backup.vdb").to_string_lossy()
            )
            .is_ok()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"synthetic encrypted vault");
    }
}
