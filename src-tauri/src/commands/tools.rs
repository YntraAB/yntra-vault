use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use serde::{Deserialize, Serialize};
use tauri::State;
use zeroize::{Zeroize, Zeroizing};

use yntra_vault_core::breach::{self, strength};
use yntra_vault_core::generator::{self, GeneratorOptions};
use yntra_vault_core::totp::{self, TotpCode, TotpConfig};
use yntra_vault_core::vault::types::{SecurityAudit, StrengthScore};

use super::AppState;

#[tauri::command]
pub async fn generate_totp(mut secret: String) -> Result<TotpCode, String> {
    let config = TotpConfig {
        secret: secret.clone(),
        ..Default::default()
    };
    let res = totp::generate_totp(&config).map_err(|e| e.to_string());
    secret.zeroize();
    res
}

#[tauri::command]
pub async fn generate_totp_with_config(mut config: TotpConfig) -> Result<TotpCode, String> {
    let res = totp::generate_totp(&config).map_err(|e| e.to_string());
    config.secret.zeroize();
    res
}

#[tauri::command]
pub async fn parse_otpauth_uri(uri: String) -> Result<TotpConfig, String> {
    totp::parse_otpauth_uri(&uri).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_password(options: GeneratorOptions) -> Result<String, String> {
    Ok(generator::generate_password(&options))
}

#[tauri::command]
pub async fn generate_password_default() -> Result<String, String> {
    Ok(generator::generate_password(&GeneratorOptions::default()))
}

#[tauri::command]
pub async fn check_password_breach(mut password: String) -> Result<breach::BreachResult, String> {
    let res = breach::check_password_breach(&password).await.map_err(|e| e.to_string());
    password.zeroize();
    res
}

#[tauri::command]
pub async fn analyze_password_strength(mut password: String) -> Result<StrengthScore, String> {
    let score = strength::analyze_password(&password);
    password.zeroize();
    Ok(score)
}

#[tauri::command]
pub async fn security_audit(state: State<'_, AppState>) -> Result<SecurityAudit, String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    manager.security_audit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_vault_file_exists(path: String) -> bool {
    Path::new(&path).exists()
}

#[tauri::command]
pub fn show_in_explorer(path: String) -> Result<(), String> {
    let target_path = Path::new(&path);
    if !target_path.exists() {
        return Err(format!("File or directory does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let mut cmd = Command::new("explorer");
        if target_path.is_dir() {
            cmd.arg(path.replace('/', "\\"));
        } else {
            cmd.arg(format!("/select,{}", path.replace('/', "\\")));
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        use std::process::Command;
        let dest = if target_path.is_dir() {
            target_path
        } else {
            target_path.parent().unwrap_or(target_path)
        };
        Command::new("xdg-open")
            .arg(dest)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
    pub category: String,
    pub is_system: bool,
}

#[tauri::command]
pub async fn get_installed_apps() -> Result<Vec<InstalledApp>, String> {
    let mut apps = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let dirs_to_scan = vec![
            std::env::var("APPDATA").ok().map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            std::env::var("ProgramData").ok().map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
        ];

        for dir in dirs_to_scan.into_iter().flatten() {
            let path = Path::new(&dir);
            if path.exists() {
                scan_dir_for_apps(path, &mut apps, 0);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mac_dirs = vec!["/Applications", "/System/Applications"];
        for dir in mac_dirs {
            let path = Path::new(dir);
            if path.exists() {
                scan_dir_for_apps(path, &mut apps, 0);
            }
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let linux_dirs = vec!["/usr/share/applications", "/usr/local/share/applications"];
        for dir in linux_dirs {
            let path = Path::new(dir);
            if path.exists() {
                scan_dir_for_apps(path, &mut apps, 0);
            }
        }
    }

    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase() || a.path == b.path);

    Ok(apps)
}

fn categorize_app(name: &str, path: &str) -> (String, bool) {
    let lower_name = name.to_lowercase();
    let lower_path = path.to_lowercase();

    // Cross-platform system utility noise filter (Windows, macOS, Linux)
    let system_noise = [
        // Windows system noise & non-login utilities
        "calculator", "character map", "snipping tool", "snip & sketch", "paint", "wordpad",
        "defragment", "disk cleanup", "component services", "computer management", "event viewer",
        "memory diagnostic", "services", "system information", "system configuration",
        "task manager", "registry editor", "regedit", "control panel", "windows defender",
        "windows security", "narrator", "magnifier", "on-screen keyboard", "voice recorder",
        "steps recorder", "windows fax", "windows media player", "quick assist", "math input panel",
        "administrative tools", "windows accessories", "windows system", "windows powershell",
        "windows kits", "accessibility", "accessability", "command prompt", "task scheduler", "taskschd", "python", "pydoc", "sample uwp",
        "recoverydrive", "recovery drive", "performance monitor", "perfmon", "livecaptions",
        "live captions", "iscsi", "git bash", "git gui", "release notes", "dfrgui",
        "application verifier", "appverif", "file explorer", "odbc", "eventvwr", "resmon",
        "resource monitor", "sysedit", "msconfig", "dxdiag", "cleanmgr", "compmgmt",
        "hyper-v", "print management", "debuggable package", "wsl", "windows subsystem for linux",
        "run", "7-zip", "7zip", "node.js", "nodejs", "autohotkey",
        // macOS system noise
        "activity monitor", "audio midi setup", "bluetooth file exchange", "colorsync utility",
        "console", "digital color meter", "disk utility", "grapher", "keychain access",
        "migration assistant", "screen sharing", "system settings", "system preferences",
        "font book", "image capture", "launchpad", "mission control", "stickies", "stocks",
        "voice memos", "automator", "chess", "dictionary", "clock",
        // Linux system noise
        "gnome-calculator", "gnome-disks", "gnome-system", "gnome-logs", "gnome-font",
        "xfce4-taskmanager", "baobab", "gparted", "nm-connection-editor", "dconf-editor",
        "system-config-", "htop", "top",
    ];

    let is_system = system_noise.iter().any(|&s| lower_name.contains(s) || lower_path.contains(s)) ||
        lower_path.contains("system32") ||
        lower_path.contains("windows\\syswow64") ||
        lower_path.contains("/system/applications/") ||
        lower_path.contains("/applications/utilities/");

    // Browsers & Communication
    if lower_name.contains("chrome") || lower_name.contains("firefox") || lower_name.contains("edge") ||
       lower_name.contains("brave") || lower_name.contains("opera") || lower_name.contains("vivaldi") ||
       lower_name.contains("tor browser") || lower_name.contains("arc") || lower_name.contains("safari") ||
       lower_name.contains("duckduckgo") || lower_name.contains("waterfox") || lower_name.contains("librewolf") ||
       lower_name.contains("discord") || lower_name.contains("slack") || lower_name.contains("telegram") ||
       lower_name.contains("whatsapp") || lower_name.contains("signal") || lower_name.contains("teams") ||
       lower_name.contains("outlook") || lower_name.contains("thunderbird") || lower_name.contains("zoom") ||
       lower_name.contains("skype") || lower_name.contains("viber") || lower_name.contains("messenger") ||
       lower_name.contains("element") || lower_name.contains("session") || lower_name.contains("mailspring") ||
       lower_path.contains("discord://") || lower_path.contains("tg://") || lower_path.contains("slack://") {
        return ("browsers_communication".into(), is_system);
    }

    // Developer, Cloud & Productivity
    if lower_name.contains("code") || lower_name.contains("cursor") || lower_name.contains("antigravity") ||
       lower_name.contains("windsurf") || lower_name.contains("zed") || lower_name.contains("vscodium") ||
       lower_name.contains("visual studio") || lower_name.contains("intellij") || lower_name.contains("datagrip") ||
       lower_name.contains("rustrover") || lower_name.contains("pycharm") || lower_name.contains("webstorm") ||
       lower_name.contains("rider") || lower_name.contains("goland") || lower_name.contains("phpstorm") ||
       lower_name.contains("clion") || lower_name.contains("android studio") || lower_name.contains("sublime") ||
       lower_name.contains("notepad++") || lower_name.contains("xcode") || lower_name.contains("fleet") ||
       lower_name.contains("dbeaver") || lower_name.contains("tableplus") || lower_name.contains("pgadmin") ||
       lower_name.contains("workbench") || lower_name.contains("compass") || lower_name.contains("redis") ||
       lower_name.contains("postman") || lower_name.contains("insomnia") || lower_name.contains("bruno") ||
       lower_name.contains("warp") || lower_name.contains("sourcetree") || lower_name.contains("gitkraken") ||
       lower_name.contains("github desktop") || lower_name.contains("docker") || lower_name.contains("filezilla") ||
       lower_name.contains("termius") || lower_name.contains("obsidian") || lower_name.contains("notion") ||
       lower_name.contains("logseq") || lower_name.contains("joplin") || lower_name.contains("evernote") ||
       lower_name.contains("onenote") || lower_name.contains("linear") || lower_name.contains("clickup") ||
       lower_name.contains("jira") || lower_name.contains("confluence") || lower_name.contains("trello") ||
       lower_name.contains("office") || lower_name.contains("word") || lower_name.contains("excel") ||
       lower_name.contains("powerpoint") || lower_name.contains("libreoffice") || lower_name.contains("figma") ||
       lower_name.contains("adobe") || lower_name.contains("creative cloud") || lower_name.contains("photoshop") ||
       lower_name.contains("illustrator") || lower_name.contains("premiere") || lower_name.contains("after effects") ||
       lower_name.contains("acrobat") || lower_name.contains("affinity") || lower_name.contains("blender") ||
       lower_name.contains("gimp") || lower_name.contains("inkscape") || lower_name.contains("davinci") ||
       lower_name.contains("wireguard") || lower_name.contains("tailscale") || lower_name.contains("mullvad") ||
       lower_name.contains("protonvpn") || lower_name.contains("nordvpn") || lower_name.contains("bitwarden") ||
       lower_name.contains("1password") || lower_name.contains("keepass") || lower_name.contains("syntra") ||
       lower_path.contains("vscode://") || lower_path.contains("cursor://") || lower_path.contains("windsurf://") ||
       lower_path.contains("obsidian://") || lower_path.contains("notion://") || lower_path.contains("linear://") {
        return ("productivity_dev".into(), is_system);
    }

    // Games, Launchers & Media
    if lower_name.contains("steam") || lower_name.contains("epic") || lower_name.contains("gog") ||
       lower_name.contains("ubisoft") || lower_name.contains("battle.net") || lower_name.contains("ea") ||
       lower_name.contains("riot") || lower_name.contains("league") || lower_name.contains("valorant") ||
       lower_name.contains("roblox") || lower_name.contains("minecraft") || lower_name.contains("geforce") ||
       lower_name.contains("nvidia") || lower_name.contains("heroic") || lower_name.contains("lutris") || lower_name.contains("prism") ||
       lower_name.contains("spotify") || lower_name.contains("tidal") || lower_name.contains("deezer") ||
       lower_name.contains("vlc") || lower_name.contains("plex") || lower_name.contains("kodi") ||
       lower_name.contains("obs studio") || lower_name.contains("audacity") || lower_name.contains("handbrake") ||
       lower_path.contains("steam://") || lower_path.contains("spotify://") {
        return ("gaming".into(), is_system);
    }

    if is_system || lower_name.contains("terminal") || lower_name.contains("powershell") || lower_name.contains("cmd") || lower_name.contains("putty") || lower_name.contains("konsole") {
        return ("system".into(), true);
    }

    ("general".into(), is_system)
}

fn scan_dir_for_apps(dir: &Path, apps: &mut Vec<InstalledApp>, depth: usize) {
    if depth > 4 { return; }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.extension().is_some_and(|ext| ext == "app") {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let path_str = p.to_string_lossy().to_string();
                    let (category, is_system) = categorize_app(&name, &path_str);
                    apps.push(InstalledApp {
                        name,
                        path: path_str,
                        category,
                        is_system,
                    });
                } else {
                    scan_dir_for_apps(&p, apps, depth + 1);
                }
            } else if let Some(ext) = p.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ext_str == "exe" || ext_str == "lnk" || ext_str == "desktop" {
                    let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let lower = name.to_lowercase();
                    if lower.contains("uninstall") || lower.contains("setup") || lower.contains("update") || lower.contains("readme") || lower.contains("help") || lower.contains("license") || lower.contains("changelog") {
                        continue;
                    }
                    let path_str = p.to_string_lossy().to_string();
                    let (category, is_system) = categorize_app(&name, &path_str);
                    apps.push(InstalledApp {
                        name,
                        path: path_str,
                        category,
                        is_system,
                    });
                }
            }
        }
    }
}

#[tauri::command]
pub async fn autotype(
    text: String,
    char_delay_ms: Option<u64>,
    settle_delay_ms: Option<u64>,
) -> Result<(), String> {
    let secret = Zeroizing::new(text);
    let char_delay = char_delay_ms.unwrap_or(15);
    let settle_delay = settle_delay_ms.unwrap_or(0);
    tokio::task::spawn_blocking(move || {
        yntra_vault_core::services::autotype::autotype_text_with_delay(&secret, char_delay, settle_delay)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_minimize_to_tray(enabled: bool, state: State<'_, AppState>) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
pub async fn run_smart_autotype(
    username: String,
    password: String,
    totp_secret: String,
    url: String,
    launch_browser: bool,
    char_delay_ms: u64,
    field_delay_ms: u64,
) -> Result<(), String> {
    yntra_vault_core::services::autotype::run_smart_autotype_with_delays(
        username,
        password,
        totp_secret,
        url,
        launch_browser,
        char_delay_ms,
        field_delay_ms,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_autostart() -> Result<(), String> {
    yntra_vault_core::services::autostart::enable_autostart().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disable_autostart() -> Result<(), String> {
    yntra_vault_core::services::autostart::disable_autostart().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn is_autostart_enabled() -> Result<bool, String> {
    yntra_vault_core::services::autostart::is_autostart_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_favicon(domain: String) -> Result<Option<String>, String> {
    yntra_vault_core::services::favicon::get_favicon(&domain)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_external_favicons_enabled(enabled: bool) -> Result<(), String> {
    yntra_vault_core::services::favicon::set_external_favicons_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub async fn is_external_favicons_enabled() -> Result<bool, String> {
    Ok(yntra_vault_core::services::favicon::is_external_favicons_enabled())
}

#[tauri::command]
pub async fn export_vault(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let source = manager.info().path;
    std::fs::copy(&source, &dest_path)
        .map_err(|e| format!("Export failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn export_vault_csv(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let path = PathBuf::from(&dest_path);
    manager.export_csv(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_vault_json(
    dest_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_ref().ok_or("Vault is locked")?;
    let path = PathBuf::from(&dest_path);
    manager.export_json(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_import_file(
    file_path: String,
    format: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::importer::ImportPreviewResult, String> {
    let requested_fmt = match format.as_deref() {
        Some("bitwarden_json") => yntra_vault_core::vault::importer::ImportFormat::BitwardenJson,
        Some("bitwarden_csv") => yntra_vault_core::vault::importer::ImportFormat::BitwardenCsv,
        Some("onepassword_csv") => yntra_vault_core::vault::importer::ImportFormat::OnePasswordCsv,
        Some("keepass_csv") => yntra_vault_core::vault::importer::ImportFormat::KeepassCsv,
        Some("keepass_xml") => yntra_vault_core::vault::importer::ImportFormat::KeepassXml,
        Some("chrome_csv") => yntra_vault_core::vault::importer::ImportFormat::ChromeCsv,
        Some("lastpass_csv") => yntra_vault_core::vault::importer::ImportFormat::LastPassCsv,
        Some("dashlane_csv") => yntra_vault_core::vault::importer::ImportFormat::DashlaneCsv,
        Some("protonpass_json") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassJson,
        Some("protonpass_csv") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassCsv,
        Some("generic_csv") => yntra_vault_core::vault::importer::ImportFormat::GenericCsv,
        _ => yntra_vault_core::vault::importer::ImportFormat::AutoDetect,
    };

    let path = PathBuf::from(&file_path);
    let mut preview = yntra_vault_core::vault::importer::Importer::parse_file(&path, requested_fmt)
        .map_err(|e| e.to_string())?;

    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref manager) = *vault {
        preview.duplicates_count = manager.check_import_duplicates(&mut preview.entries);
    }

    Ok(preview)
}

#[tauri::command]
pub async fn parse_import_content(
    content: String,
    format: Option<String>,
    state: State<'_, AppState>,
) -> Result<yntra_vault_core::vault::importer::ImportPreviewResult, String> {
    let requested_fmt = match format.as_deref() {
        Some("bitwarden_json") => yntra_vault_core::vault::importer::ImportFormat::BitwardenJson,
        Some("bitwarden_csv") => yntra_vault_core::vault::importer::ImportFormat::BitwardenCsv,
        Some("onepassword_csv") => yntra_vault_core::vault::importer::ImportFormat::OnePasswordCsv,
        Some("keepass_csv") => yntra_vault_core::vault::importer::ImportFormat::KeepassCsv,
        Some("keepass_xml") => yntra_vault_core::vault::importer::ImportFormat::KeepassXml,
        Some("chrome_csv") => yntra_vault_core::vault::importer::ImportFormat::ChromeCsv,
        Some("lastpass_csv") => yntra_vault_core::vault::importer::ImportFormat::LastPassCsv,
        Some("dashlane_csv") => yntra_vault_core::vault::importer::ImportFormat::DashlaneCsv,
        Some("protonpass_json") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassJson,
        Some("protonpass_csv") => yntra_vault_core::vault::importer::ImportFormat::ProtonPassCsv,
        Some("generic_csv") => yntra_vault_core::vault::importer::ImportFormat::GenericCsv,
        _ => yntra_vault_core::vault::importer::ImportFormat::AutoDetect,
    };

    let mut preview = yntra_vault_core::vault::importer::Importer::parse_str(&content, requested_fmt)
        .map_err(|e| e.to_string())?;

    let vault = state.vault.lock().map_err(|e| e.to_string())?;
    if let Some(ref manager) = *vault {
        preview.duplicates_count = manager.check_import_duplicates(&mut preview.entries);
    }

    Ok(preview)
}

#[tauri::command]
pub async fn import_entries(
    entries: Vec<yntra_vault_core::vault::importer::ParsedImportEntry>,
    duplicate_strategy: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let mut vault = state.vault.lock().map_err(|e| e.to_string())?;
    let manager = vault.as_mut().ok_or("Vault is locked")?;

    let strategy = match duplicate_strategy.as_str() {
        "overwrite" => yntra_vault_core::vault::importer::DuplicateStrategy::Overwrite,
        "keep_both" => yntra_vault_core::vault::importer::DuplicateStrategy::KeepBoth,
        _ => yntra_vault_core::vault::importer::DuplicateStrategy::Skip,
    };

    manager.bulk_import_entries(entries, strategy).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_to_clipboard(
    text: String,
    is_sensitive: Option<bool>,
    clear_after_secs: Option<u64>,
) -> Result<(), String> {
    let sensitive = is_sensitive.unwrap_or(true);
    yntra_vault_core::crypto::copy_to_clipboard_defended(&text, sensitive, clear_after_secs)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_clipboard() -> Result<(), String> {
    yntra_vault_core::crypto::clear_clipboard()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_window_capture_protection(
    window: tauri::WebviewWindow,
    enable: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        yntra_vault_core::crypto::set_window_capture_protection(hwnd.0 as isize, enable)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, enable);
        Ok(())
    }
}

#[tauri::command]
pub async fn set_lock_on_focus_loss(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.lock_on_focus_loss.store(enabled, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn set_lock_on_system_lock(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.lock_on_system_lock.store(enabled, Ordering::Relaxed);
    Ok(())
}
