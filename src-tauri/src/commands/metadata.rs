//! Non-secret UI metadata lives beside app data, independent of the WebView cache.
//! Never use this store for passwords, keyfile paths, recovery shares or session keys.
use std::{collections::BTreeMap, io::{Read, Write}, path::Path, sync::Mutex};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

static STORE_LOCK: Mutex<()> = Mutex::new(());
const LIMIT: u64 = 256 * 1024;
pub type MetadataValues = BTreeMap<String, String>;

#[derive(Serialize, Deserialize)]
struct Metadata { schema: u32, values: MetadataValues }

fn sanitize(mut values: MetadataValues) -> Result<MetadataValues, String> {
    if serde_json::to_vec(&values).map_err(|e| e.to_string())?.len() > LIMIT as usize {
        return Err("Application metadata is too large".into());
    }
    for (key, raw) in values.iter_mut() {
        match key.as_str() {
            "yntra-vault-recent-vaults" => {
                let parsed: serde_json::Value = serde_json::from_str(raw).map_err(|_| "Invalid recent vault list")?;
                let list = parsed.as_array().ok_or("Invalid recent vault list")?;
                if list.len() > 10 { return Err("Too many recent vaults".into()); }
                let mut clean = Vec::new();
                for vault in list {
                    let mut entry = serde_json::Map::new();
                    for field in ["id", "name", "path"] {
                        let text = vault.get(field).and_then(|v| v.as_str()).ok_or("Invalid recent vault metadata")?;
                        if text.len() > 8192 || text.contains('\0') { return Err("Invalid recent vault metadata".into()); }
                        entry.insert(field.into(), text.into());
                    }
                    clean.push(entry);
                }
                *raw = serde_json::to_string(&clean).map_err(|e| e.to_string())?;
            }
            "yntra-vault-settings" => {
                let mut settings: serde_json::Map<String, serde_json::Value> = serde_json::from_str(raw).map_err(|_| "Invalid settings")?;
                const ALLOWED: &[&str] = &[
                    "theme", "language", "sidebarWidth", "passwordListWidth", "fontSize", "density",
                    "autoLockMinutes", "clipboardClearSeconds", "minimizeToTray", "launchOnStartup",
                    "disableSkeletonDelays", "autoBreachCheck", "showBreachInList", "autotypeCharDelayMs",
                    "autotypeFieldDelayMs", "autotypeSettleDelayMs", "autotypeLaunchBrowser", "tagSortOrder",
                    "showTagCounts", "entrySortOrder", "groupByDate", "keybinds", "forceMobileView",
                    "windowCaptureProtection", "lockOnFocusLoss", "lockOnSystemLock", "webdavEnabled",
                    "webdavUrl", "webdavUser", "webdavAutoSync", "p2pAddr", "p2pAutoListen", "p2pAutoSyncWifi",
                    "p2pAutoSyncIntervalMinutes", "externalFaviconsEnabled", "autoCheckUpdates", "operationMode",
                ];
                settings.retain(|key, _| ALLOWED.contains(&key.as_str()));
                *raw = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
            }
            "yntra-vault-theme" if ["dark", "light", "system"].contains(&raw.as_str()) => {}
            "yntra-vault-setup-completed" if ["true", "false"].contains(&raw.as_str()) => {}
            _ => return Err("Unsupported application metadata key".into()),
        }
    }
    Ok(values)
}

fn read_store(path: &Path) -> Result<MetadataValues, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(_) => return Err("Cannot read saved application metadata".into()),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes).map_err(|_| "Cannot read saved application metadata")?;
    if bytes.len() > LIMIT as usize { return Err("Saved application metadata is too large".into()); }
    let metadata: Metadata = serde_json::from_slice(&bytes).map_err(|_| "Saved application metadata is damaged; it has not been reset")?;
    if metadata.schema != 1 { return Err("Application metadata needs a newer app version".into()); }
    sanitize(metadata.values)
}

fn write_store(path: &Path, values: MetadataValues, initialize: bool) -> Result<(), String> {
    let values = sanitize(values)?;
    // Refuse to silently replace damaged or newer metadata with defaults.
    let mut merged = read_store(path)?;
    for (key, value) in values {
        if initialize { merged.entry(key).or_insert(value); }
        else { merged.insert(key, value); }
    }
    let parent = path.parent().ok_or("Invalid metadata directory")?;
    std::fs::create_dir_all(parent).map_err(|_| "Cannot create application metadata directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|_| "Cannot stage application metadata")?;
    let bytes = serde_json::to_vec(&Metadata { schema: 1, values: merged }).map_err(|_| "Cannot save application metadata")?;
    if bytes.len() > LIMIT as usize { return Err("Application metadata is too large".into()); }
    file.write_all(&bytes).map_err(|_| "Cannot save application metadata")?;
    file.flush().and_then(|_| file.as_file().sync_all()).map_err(|_| "Cannot flush application metadata")?;
    file.persist(path).map_err(|_| "Cannot replace application metadata")?;
    Ok(())
}

fn lock_store(path: &Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(path.parent().ok_or("Invalid metadata directory")?).map_err(|_| "Cannot create metadata directory")?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(path.with_extension("lock")).map_err(|_| "Cannot open metadata lock")?;
    lock.lock().map_err(|_| "Cannot lock application metadata")?;
    Ok(lock) // OS lock is released when this handle is dropped, including on failure.
}

#[tauri::command]
pub fn load_ui_metadata(app: AppHandle) -> Result<MetadataValues, String> {
    let _lock = STORE_LOCK.lock().map_err(|_| "Application metadata is busy")?;
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("ui-metadata-v1.json");
    let _file_lock = lock_store(&path)?;
    read_store(&path)
}

#[tauri::command]
pub fn save_ui_metadata(app: AppHandle, values: MetadataValues, initialize: Option<bool>) -> Result<(), String> {
    let _lock = STORE_LOCK.lock().map_err(|_| "Application metadata is busy")?;
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("ui-metadata-v1.json");
    let _file_lock = lock_store(&path)?;
    write_store(&path, values, initialize.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_survives_reload_without_retaining_unlock_factors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-metadata-v1.json");
        let mut values = BTreeMap::new();
        values.insert("yntra-vault-recent-vaults".into(), r#"[{"id":"id","name":"Personal","path":"E:\\vault.vdb","keyFilePath":"secret.key","password":"secret"}]"#.into());
        values.insert("yntra-vault-settings".into(), r#"{"autoLockMinutes":5,"language":"sv","password":"secret"}"#.into());
        write_store(&path, values, false).unwrap();
        let loaded = read_store(&path).unwrap();
        assert!(loaded["yntra-vault-recent-vaults"].contains("vault.vdb"));
        assert!(loaded["yntra-vault-settings"].contains("autoLockMinutes"));
        assert!(!std::fs::read_to_string(&path).unwrap().contains("secret"));
        write_store(&path, loaded.clone(), false).unwrap();
        assert_eq!(loaded, read_store(&path).unwrap());
    }
    #[test]
    fn invalid_newer_or_damaged_metadata_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("metadata.json");
        for existing in [r#"{"schema":2,"values":{}}"#, "broken"] {
            std::fs::write(&path, existing).unwrap();
            assert!(write_store(&path, BTreeMap::new(), false).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), existing);
        }
        assert!(sanitize(BTreeMap::from([("yntra-vault-keyfiles".into(), "secret".into())])).is_err());
    }
    #[test]
    fn preferences_do_not_overwrite_another_instances_vault_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("metadata.json");
        let _lock = lock_store(&path).unwrap();
        let vaults = BTreeMap::from([("yntra-vault-recent-vaults".into(), "[]".into())]);
        write_store(&path, vaults.clone(), false).unwrap();
        write_store(&path, BTreeMap::from([("yntra-vault-theme".into(), "dark".into())]), false).unwrap();
        write_store(&path, BTreeMap::from([("yntra-vault-theme".into(), "light".into())]), true).unwrap();
        let loaded = read_store(&path).unwrap();
        assert_eq!(loaded["yntra-vault-recent-vaults"], "[]");
        assert_eq!(loaded["yntra-vault-theme"], "dark");
    }
}
