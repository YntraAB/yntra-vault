//! Competitor Importer Module
//!
//! Provides multi-format parsing for Bitwarden, 1Password, KeePass/KeePassXC,
//! Chrome/Edge/Brave/Firefox, LastPass, Dashlane, Proton Pass, and Generic CSV formats.

use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::vault::types::{CustomField, EntryType, FieldType};

/// Supported import source formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    AutoDetect,
    BitwardenJson,
    BitwardenCsv,
    OnePasswordCsv,
    KeepassCsv,
    KeepassXml,
    ChromeCsv,
    LastPassCsv,
    DashlaneCsv,
    ProtonPassJson,
    ProtonPassCsv,
    GenericCsv,
}

impl Default for ImportFormat {
    fn default() -> Self {
        ImportFormat::AutoDetect
    }
}

/// A parsed entry ready for preview or vault insertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedImportEntry {
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub email: String,
    pub notes: String,
    pub totp_secret: Option<String>,
    pub custom_fields: Vec<CustomField>,
    pub entry_type: EntryType,
    pub tags: Vec<String>,
    pub is_duplicate: bool,
    pub duplicate_reason: Option<String>,
}

/// Result of parsing an import file, returned for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewResult {
    pub format_detected: String,
    pub detected_format_key: String,
    pub is_format_mismatch: bool,
    pub suggested_brand_name: Option<String>,
    pub total_found: usize,
    pub entries: Vec<ParsedImportEntry>,
    pub duplicates_count: usize,
}

/// Strategy for handling duplicate entries during import execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateStrategy {
    Skip,
    Overwrite,
    KeepBoth,
}

// ─── CSV Parser Implementation (RFC 4180 compliant) ──────────────────────

/// Parses a CSV string into a 2D matrix of row cells, respecting quotes & escaped quotes.
pub fn parse_csv_matrix(content: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote ("")
                    current_field.push('"');
                    chars.next();
                } else {
                    // Closing quote
                    in_quotes = false;
                }
            } else {
                current_field.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => {
                    current_row.push(current_field.trim().to_string());
                    current_field = String::new();
                }
                '\n' => {
                    current_row.push(current_field.trim().to_string());
                    if !current_row.iter().all(|f| f.is_empty()) {
                        rows.push(current_row);
                    }
                    current_row = Vec::new();
                    current_field = String::new();
                }
                '\r' => {
                    // Skip carriage return
                    if chars.peek() == Some(&'\n') {}
                }
                _ => current_field.push(c),
            }
        }
    }

    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field.trim().to_string());
        if !current_row.iter().all(|f| f.is_empty()) {
            rows.push(current_row);
        }
    }

    rows
}

/// Helper function to clean TOTP secret (extracts secret parameter if full otpauth:// URI).
pub fn clean_totp_secret(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("otpauth://") {
        if let Some(pos) = trimmed.find("secret=") {
            let sub = &trimmed[pos + 7..];
            let secret = sub.split('&').next().unwrap_or("").trim();
            if !secret.is_empty() {
                return Some(secret.to_string());
            }
        }
    }

    // Clean space/dash formatting inside raw base32 secret
    let cleaned = trimmed.replace([' ', '-'], "").to_uppercase();
    if !cleaned.is_empty() {
        Some(cleaned)
    } else {
        None
    }
}

// ─── Importer Logic ──────────────────────────────────────────────────────

pub struct Importer;

impl Importer {
    /// Reads and parses an import file according to the requested format.
    /// Employs Zeroizing buffer to ensure raw credentials are deleted from RAM on completion.
    pub fn parse_file(path: &Path, requested_format: ImportFormat) -> crate::Result<ImportPreviewResult> {
        let bytes = std::fs::read(path)
            .map_err(|e| crate::VaultError::InvalidFormat(format!("Failed to read import file: {}", e)))?;

        // Inspect binary magic headers for friendly user error messaging
        if bytes.starts_with(&[0x03, 0xd9, 0xa2, 0x9a]) || bytes.starts_with(&[0x67, 0xfb, 0x4b, 0xb5]) {
            return Err(crate::VaultError::InvalidFormat(
                "KeePass .kdbx files are encrypted binary databases. Please export your database from KeePass/KeePassXC as an XML or CSV file.".to_string()
            ));
        }

        if bytes.starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
            return Err(crate::VaultError::InvalidFormat(
                "1PUX / ZIP archives are compressed files. Please export your items as CSV, or extract the archive first.".to_string()
            ));
        }

        let raw_content = String::from_utf8(bytes)
            .map_err(|_| crate::VaultError::InvalidFormat("The file contains binary data and cannot be parsed as a text password export.".to_string()))?;

        let zero_content = Zeroizing::new(raw_content);
        Self::parse_str(&zero_content, requested_format)
    }

    /// Parses string content into raw entries with format mismatch detection & auto-recovery.
    pub fn parse_str(content: &str, requested_format: ImportFormat) -> crate::Result<ImportPreviewResult> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(crate::VaultError::InvalidFormat("The imported file is empty.".to_string()));
        }

        let auto_detected = Self::detect_format(trimmed);

        let target_format = if requested_format == ImportFormat::AutoDetect {
            auto_detected
        } else {
            requested_format
        };

        // Try parsing with requested format first
        let primary_parse = Self::execute_parse(trimmed, target_format);

        let (final_format, entries, is_mismatch, suggested_brand) = match primary_parse {
            Ok(res) if !res.is_empty() => {
                let mismatch = requested_format != ImportFormat::AutoDetect
                    && requested_format != auto_detected
                    && auto_detected != ImportFormat::GenericCsv;

                let brand_name = if mismatch {
                    Some(Self::format_to_brand_name(auto_detected).to_string())
                } else {
                    None
                };

                (target_format, res, mismatch, brand_name)
            }
            _ => {
                // Primary format returned 0 items or failed. Fallback to auto-detected format if different.
                if target_format != auto_detected {
                    if let Ok(fallback_entries) = Self::execute_parse(trimmed, auto_detected) {
                        if !fallback_entries.is_empty() {
                            let brand_name = Self::format_to_brand_name(auto_detected).to_string();
                            (auto_detected, fallback_entries, true, Some(brand_name))
                        } else {
                            (target_format, Vec::new(), false, None)
                        }
                    } else {
                        (target_format, Vec::new(), false, None)
                    }
                } else {
                    (target_format, Vec::new(), false, None)
                }
            }
        };

        let total_found = entries.len();
        let format_label = Self::format_to_label(final_format);
        let format_key = Self::format_to_key(final_format);

        Ok(ImportPreviewResult {
            format_detected: format_label.to_string(),
            detected_format_key: format_key.to_string(),
            is_format_mismatch: is_mismatch,
            suggested_brand_name: suggested_brand,
            total_found,
            entries,
            duplicates_count: 0,
        })
    }

    fn execute_parse(trimmed: &str, format: ImportFormat) -> crate::Result<Vec<ParsedImportEntry>> {
        let mut entries = match format {
            ImportFormat::BitwardenJson => Self::parse_bitwarden_json(trimmed)?,
            ImportFormat::BitwardenCsv => Self::parse_bitwarden_csv(trimmed)?,
            ImportFormat::OnePasswordCsv => Self::parse_1password_csv(trimmed)?,
            ImportFormat::KeepassCsv => Self::parse_keepass_csv(trimmed)?,
            ImportFormat::KeepassXml => Self::parse_keepass_xml(trimmed)?,
            ImportFormat::ChromeCsv => Self::parse_chrome_csv(trimmed)?,
            ImportFormat::LastPassCsv => Self::parse_lastpass_csv(trimmed)?,
            ImportFormat::DashlaneCsv => Self::parse_dashlane_csv(trimmed)?,
            ImportFormat::ProtonPassJson => Self::parse_protonpass_json(trimmed)?,
            ImportFormat::ProtonPassCsv => Self::parse_protonpass_csv(trimmed)?,
            ImportFormat::GenericCsv | ImportFormat::AutoDetect => Self::parse_generic_csv(trimmed)?,
        };

        // Filter out completely empty entries
        entries.retain(|e| !e.title.is_empty() || !e.username.is_empty() || !e.password.is_empty() || !e.notes.is_empty());
        Ok(entries)
    }

    fn format_to_label(format: ImportFormat) -> &'static str {
        match format {
            ImportFormat::BitwardenJson => "Bitwarden (JSON)",
            ImportFormat::BitwardenCsv => "Bitwarden (CSV)",
            ImportFormat::OnePasswordCsv => "1Password (CSV)",
            ImportFormat::KeepassCsv => "KeePass (CSV)",
            ImportFormat::KeepassXml => "KeePass (XML)",
            ImportFormat::ChromeCsv => "Browser Passwords (Chrome/Edge/Firefox CSV)",
            ImportFormat::LastPassCsv => "LastPass (CSV)",
            ImportFormat::DashlaneCsv => "Dashlane (CSV)",
            ImportFormat::ProtonPassJson => "Proton Pass (JSON)",
            ImportFormat::ProtonPassCsv => "Proton Pass (CSV)",
            ImportFormat::GenericCsv | ImportFormat::AutoDetect => "Generic Password CSV",
        }
    }

    fn format_to_key(format: ImportFormat) -> &'static str {
        match format {
            ImportFormat::BitwardenJson => "bitwarden_json",
            ImportFormat::BitwardenCsv => "bitwarden_csv",
            ImportFormat::OnePasswordCsv => "onepassword_csv",
            ImportFormat::KeepassCsv => "keepass_csv",
            ImportFormat::KeepassXml => "keepass_xml",
            ImportFormat::ChromeCsv => "chrome_csv",
            ImportFormat::LastPassCsv => "lastpass_csv",
            ImportFormat::DashlaneCsv => "dashlane_csv",
            ImportFormat::ProtonPassJson => "protonpass_json",
            ImportFormat::ProtonPassCsv => "protonpass_csv",
            ImportFormat::GenericCsv | ImportFormat::AutoDetect => "generic_csv",
        }
    }

    fn format_to_brand_name(format: ImportFormat) -> &'static str {
        match format {
            ImportFormat::BitwardenJson | ImportFormat::BitwardenCsv => "Bitwarden",
            ImportFormat::OnePasswordCsv => "1Password",
            ImportFormat::KeepassCsv | ImportFormat::KeepassXml => "KeePass",
            ImportFormat::ChromeCsv => "Google Chrome / Edge",
            ImportFormat::LastPassCsv => "LastPass",
            ImportFormat::DashlaneCsv => "Dashlane",
            ImportFormat::ProtonPassJson | ImportFormat::ProtonPassCsv => "Proton Pass",
            ImportFormat::GenericCsv | ImportFormat::AutoDetect => "Generic CSV",
        }
    }

    /// Auto-detects the format from raw content.
    fn detect_format(content: &str) -> ImportFormat {
        if content.starts_with('{') {
            if content.contains("\"items\"") || content.contains("\"encrypted\"") {
                return ImportFormat::BitwardenJson;
            }
            if content.contains("\"vaults\"") || content.contains("\"item\"") {
                return ImportFormat::ProtonPassJson;
            }
            return ImportFormat::BitwardenJson;
        }

        if content.starts_with('<') || content.contains("<KeePassFile>") {
            return ImportFormat::KeepassXml;
        }

        let matrix = parse_csv_matrix(content);
        if let Some(header) = matrix.first() {
            let h_lower: Vec<String> = header.iter().map(|s| s.to_lowercase()).collect();

            if h_lower.iter().any(|h| h == "folder") && h_lower.iter().any(|h| h == "login_username") {
                return ImportFormat::BitwardenCsv;
            }
            if h_lower.iter().any(|h| h == "group") && h_lower.iter().any(|h| h == "title") && h_lower.iter().any(|h| h == "password") {
                return ImportFormat::KeepassCsv;
            }
            if h_lower.iter().any(|h| h == "title") && (h_lower.iter().any(|h| h == "otp") || h_lower.iter().any(|h| h == "one-time password")) {
                return ImportFormat::OnePasswordCsv;
            }
            if h_lower.iter().any(|h| h == "grouping") && h_lower.iter().any(|h| h == "extra") {
                return ImportFormat::LastPassCsv;
            }
            if h_lower.iter().any(|h| h == "name") && h_lower.iter().any(|h| h == "url") && h_lower.iter().any(|h| h == "username") && h_lower.iter().any(|h| h == "password") {
                return ImportFormat::ChromeCsv;
            }
        }

        ImportFormat::GenericCsv
    }

    // ─── Format Parsers ───────────────────────────────────────────────────

    /// Bitwarden JSON parser (supports custom fields, item types, & multiple URIs)
    fn parse_bitwarden_json(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let json: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| crate::VaultError::InvalidFormat(format!("Invalid Bitwarden JSON: {}", e)))?;

        if json.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false)
            || (json.get("encrypted").is_some() && json.get("items").is_none())
        {
            return Err(crate::VaultError::InvalidFormat(
                "Bitwarden Encrypted JSON exports are protected by account-specific encryption keys. Please re-export your vault from Bitwarden as an unencrypted JSON or CSV.".to_string(),
            ));
        }

        let mut result = Vec::new();

        if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
            for item in items {
                let title = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let notes = item.get("notes").and_then(|v| v.as_str()).unwrap_or("").to_string();

                let item_type_num = item.get("type").and_then(|v| v.as_u64()).unwrap_or(1);
                let entry_type = match item_type_num {
                    2 => EntryType::SecureNote,
                    3 => EntryType::CreditCard,
                    4 => EntryType::Identity,
                    _ => EntryType::Login,
                };
                
                let mut username = String::new();
                let mut password = String::new();
                let mut url = String::new();
                let mut totp = None;
                let mut custom_fields = Vec::new();

                if let Some(login) = item.get("login") {
                    username = login.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    password = login.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if let Some(t) = login.get("totp").and_then(|v| v.as_str()) {
                        totp = clean_totp_secret(t);
                    }
                    if let Some(uris) = login.get("uris").and_then(|v| v.as_array()) {
                        for (u_idx, u_obj) in uris.iter().enumerate() {
                            if let Some(u_str) = u_obj.get("uri").and_then(|v| v.as_str()) {
                                if u_idx == 0 {
                                    url = u_str.to_string();
                                } else {
                                    custom_fields.push(CustomField {
                                        id: Uuid::new_v4(),
                                        name: format!("Alternative URI {}", u_idx + 1),
                                        field_type: FieldType::Url,
                                        value: u_str.to_string(),
                                        sensitive: false,
                                    });
                                }
                            }
                        }
                    }
                }

                // Bitwarden custom fields array parsing
                if let Some(fields) = item.get("fields").and_then(|f| f.as_array()) {
                    for f in fields {
                        let f_name = f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let f_val = f.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let f_type_num = f.get("type").and_then(|v| v.as_u64()).unwrap_or(0);

                        if !f_name.is_empty() || !f_val.is_empty() {
                            let (ft, sensitive) = match f_type_num {
                                1 => (FieldType::Password, true),
                                2 => (FieldType::Text, false),
                                _ => (FieldType::Text, false),
                            };
                            custom_fields.push(CustomField {
                                id: Uuid::new_v4(),
                                name: if f_name.is_empty() { "Custom Field".to_string() } else { f_name },
                                field_type: ft,
                                value: f_val,
                                sensitive,
                            });
                        }
                    }
                }

                let email = if username.contains('@') { username.clone() } else { String::new() };

                result.push(ParsedImportEntry {
                    title,
                    username,
                    password,
                    url,
                    email,
                    notes,
                    totp_secret: totp,
                    custom_fields,
                    entry_type,
                    tags: vec!["Bitwarden".to_string()],
                    is_duplicate: false,
                    duplicate_reason: None,
                });
            }
        }

        Ok(result)
    }

    /// Bitwarden CSV parser
    fn parse_bitwarden_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["name", "title"]);
            let username = idx.get(row, &["login_username", "username"]);
            let password = idx.get(row, &["login_password", "password"]);
            let url = idx.get(row, &["login_uri", "url", "website"]);
            let notes = idx.get(row, &["notes", "note"]);
            let totp_raw = idx.get(row, &["login_totp", "totp"]);
            let folder = idx.get(row, &["folder"]);

            let email = if username.contains('@') { username.clone() } else { String::new() };
            let totp_secret = clean_totp_secret(&totp_raw);

            let mut tags = vec!["Bitwarden".to_string()];
            if !folder.is_empty() {
                tags.push(folder);
            }

            result.push(ParsedImportEntry {
                title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags,
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// 1Password CSV parser
    fn parse_1password_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["title", "name"]);
            let username = idx.get(row, &["username", "user", "email"]);
            let password = idx.get(row, &["password", "pass"]);
            let url = idx.get(row, &["url", "website"]);
            let notes = idx.get(row, &["notes", "note", "comments"]);
            let totp_raw = idx.get(row, &["otp", "one-time password", "totp"]);

            let email = if username.contains('@') { username.clone() } else { String::new() };
            let totp_secret = clean_totp_secret(&totp_raw);

            result.push(ParsedImportEntry {
                title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags: vec!["1Password".to_string()],
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// KeePass / KeePassXC CSV parser
    fn parse_keepass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["title"]);
            let username = idx.get(row, &["username", "user_name"]);
            let password = idx.get(row, &["password"]);
            let url = idx.get(row, &["url"]);
            let notes = idx.get(row, &["notes", "comment"]);
            let group = idx.get(row, &["group"]);
            let totp_raw = idx.get(row, &["totp"]);

            let email = if username.contains('@') { username.clone() } else { String::new() };
            let totp_secret = clean_totp_secret(&totp_raw);

            let mut tags = vec!["KeePass".to_string()];
            if !group.is_empty() {
                tags.push(group);
            }

            result.push(ParsedImportEntry {
                title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags,
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// KeePass XML parser (extracts title, username, password, url, notes, TOTP, and Group tags)
    fn parse_keepass_xml(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let mut result = Vec::new();

        let mut current_group = String::new();

        for chunk in content.split("<Group>") {
            if chunk.contains("<Name>") && chunk.contains("</Name>") {
                if let Some(n_start) = chunk.find("<Name>") {
                    let rest = &chunk[n_start + 6..];
                    if let Some(n_end) = rest.find("</Name>") {
                        let gname = rest[..n_end].trim();
                        if !gname.is_empty() && gname != "Root" {
                            current_group = gname.to_string();
                        }
                    }
                }
            }

            for entry_chunk in chunk.split("<Entry>") {
                if !entry_chunk.contains("</Entry>") {
                    continue;
                }
                let block = entry_chunk.split("</Entry>").next().unwrap_or("");

                let title = extract_xml_key_value(block, "Title");
                let username = extract_xml_key_value(block, "UserName");
                let password = extract_xml_key_value(block, "Password");
                let url = extract_xml_key_value(block, "URL");
                let notes = extract_xml_key_value(block, "Notes");
                let totp_raw = extract_xml_key_value(block, "TimeOtp-Secret-Base32");
                let totp_secret = clean_totp_secret(&totp_raw);

                if !title.is_empty() || !username.is_empty() || !password.is_empty() {
                    let email = if username.contains('@') { username.clone() } else { String::new() };
                    let mut tags = vec!["KeePass".to_string()];
                    if !current_group.is_empty() {
                        tags.push(current_group.clone());
                    }

                    result.push(ParsedImportEntry {
                        title,
                        username,
                        password,
                        url,
                        email,
                        notes,
                        totp_secret,
                        custom_fields: Vec::new(),
                        entry_type: EntryType::Login,
                        tags,
                        is_duplicate: false,
                        duplicate_reason: None,
                    });
                }
            }
        }

        Ok(result)
    }

    /// Chrome / Edge / Firefox CSV parser
    fn parse_chrome_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["name", "title"]);
            let url = idx.get(row, &["url", "url_href"]);
            let username = idx.get(row, &["username", "user"]);
            let password = idx.get(row, &["password", "pass"]);
            let notes = idx.get(row, &["note", "notes"]);

            let display_title = if !title.is_empty() {
                title
            } else if !url.is_empty() {
                url.clone()
            } else {
                "Imported Account".to_string()
            };

            let email = if username.contains('@') { username.clone() } else { String::new() };

            result.push(ParsedImportEntry {
                title: display_title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret: None,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags: vec!["Browser".to_string()],
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// LastPass CSV parser
    fn parse_lastpass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let url = idx.get(row, &["url"]);
            let username = idx.get(row, &["username"]);
            let password = idx.get(row, &["password"]);
            let notes = idx.get(row, &["extra", "notes"]);
            let name = idx.get(row, &["name"]);
            let grouping = idx.get(row, &["grouping"]);

            let title = if !name.is_empty() { name } else { url.clone() };
            let email = if username.contains('@') { username.clone() } else { String::new() };

            let mut tags = vec!["LastPass".to_string()];
            if !grouping.is_empty() {
                tags.push(grouping);
            }

            result.push(ParsedImportEntry {
                title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret: None,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags,
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// Dashlane CSV parser
    fn parse_dashlane_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["title", "name"]);
            let username = idx.get(row, &["username", "login"]);
            let password = idx.get(row, &["password"]);
            let url = idx.get(row, &["url"]);
            let notes = idx.get(row, &["note", "notes"]);

            let email = if username.contains('@') { username.clone() } else { String::new() };

            result.push(ParsedImportEntry {
                title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret: None,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags: vec!["Dashlane".to_string()],
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// Proton Pass JSON parser
    fn parse_protonpass_json(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let json: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| crate::VaultError::InvalidFormat(format!("Invalid Proton Pass JSON: {}", e)))?;

        let mut result = Vec::new();

        if let Some(vaults) = json.get("vaults").and_then(|v| v.as_array()) {
            for vault in vaults {
                if let Some(items) = vault.get("items").and_then(|i| i.as_array()) {
                    for item in items {
                        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let notes = item.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        
                        let mut username = String::new();
                        let mut password = String::new();
                        let mut url = String::new();
                        let mut totp = None;

                        if let Some(data) = item.get("data") {
                            username = data.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            password = data.get("password").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if let Some(t) = data.get("totpUri").or_else(|| data.get("totp")).and_then(|v| v.as_str()) {
                                totp = clean_totp_secret(t);
                            }
                            if let Some(urls) = data.get("urls").and_then(|v| v.as_array()) {
                                if let Some(u) = urls.first().and_then(|v| v.as_str()) {
                                    url = u.to_string();
                                }
                            }
                        }

                        let email = if username.contains('@') { username.clone() } else { String::new() };

                        result.push(ParsedImportEntry {
                            title,
                            username,
                            password,
                            url,
                            email,
                            notes,
                            totp_secret: totp,
                            custom_fields: Vec::new(),
                            entry_type: EntryType::Login,
                            tags: vec!["ProtonPass".to_string()],
                            is_duplicate: false,
                            duplicate_reason: None,
                        });
                    }
                }
            }
        }

        Ok(result)
    }

    /// Proton Pass CSV parser
    fn parse_protonpass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        Self::parse_generic_csv(content)
    }

    /// Generic CSV parser with fuzzy column matching and positional index fallback
    fn parse_generic_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_matrix(content);
        if matrix.is_empty() {
            return Ok(Vec::new());
        }

        // If single row without header, or matrix >= 2
        let has_header = matrix.len() >= 2;
        let start_row = if has_header { 1 } else { 0 };

        let idx = if has_header {
            HeaderIndex::new(&matrix[0])
        } else {
            HeaderIndex::new(&[])
        };

        let mut result = Vec::new();
        for row in matrix.iter().skip(start_row) {
            let mut title = idx.get(row, &["title", "name", "item", "service", "account"]);
            let mut username = idx.get(row, &["username", "user", "login", "email"]);
            let mut password = idx.get(row, &["password", "pass", "secret"]);
            let mut url = idx.get(row, &["url", "website", "link", "uri"]);
            let mut notes = idx.get(row, &["notes", "note", "comment", "description"]);
            let totp_raw = idx.get(row, &["totp", "otp", "totp_secret"]);

            // Positional index fallback if headers were unmapped
            if title.is_empty() && username.is_empty() && password.is_empty() {
                if row.len() > 0 { title = row[0].trim().to_string(); }
                if row.len() > 1 { username = row[1].trim().to_string(); }
                if row.len() > 2 { password = row[2].trim().to_string(); }
                if row.len() > 3 { url = row[3].trim().to_string(); }
                if row.len() > 4 { notes = row[4].trim().to_string(); }
            }

            let email = if username.contains('@') { username.clone() } else { String::new() };
            let totp_secret = clean_totp_secret(&totp_raw);

            let display_title = if !title.is_empty() {
                title
            } else if !url.is_empty() {
                url.clone()
            } else if !username.is_empty() {
                username.clone()
            } else {
                "Imported Item".to_string()
            };

            result.push(ParsedImportEntry {
                title: display_title,
                username,
                password,
                url,
                email,
                notes,
                totp_secret,
                custom_fields: Vec::new(),
                entry_type: EntryType::Login,
                tags: vec!["Imported".to_string()],
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }
}

// ─── Header Indexing Helper ──────────────────────────────────────────────

struct HeaderIndex {
    map: std::collections::HashMap<String, usize>,
}

impl HeaderIndex {
    fn new(header: &[String]) -> Self {
        let mut map = std::collections::HashMap::new();
        for (i, h) in header.iter().enumerate() {
            let clean = h.trim().to_lowercase().replace([' ', '_', '-'], "");
            map.insert(clean, i);
        }
        Self { map }
    }

    fn get(&self, row: &[String], candidates: &[&str]) -> String {
        // 1. Try exact matches first
        for cand in candidates {
            let clean = cand.replace([' ', '_', '-'], "");
            if let Some(&idx) = self.map.get(&clean) {
                if idx < row.len() {
                    let val = row[idx].trim();
                    if !val.is_empty() {
                        return val.to_string();
                    }
                }
            }
        }

        // 2. Try substring / partial matches
        for cand in candidates {
            let clean = cand.replace([' ', '_', '-'], "");
            if clean.len() < 3 {
                continue;
            }
            for (key, &idx) in &self.map {
                if (key.contains(&clean) || clean.contains(key)) && idx < row.len() {
                    let val = row[idx].trim();
                    if !val.is_empty() {
                        return val.to_string();
                    }
                }
            }
        }

        String::new()
    }
}

/// Helper function to extract `<Key Name="key">val</Key>` or `<key>val</key>` from KeePass XML string.
fn extract_xml_key_value(block: &str, key: &str) -> String {
    let key_pattern = format!("<Key>{}</Key>", key);
    if let Some(pos) = block.find(&key_pattern) {
        let rest = &block[pos + key_pattern.len()..];
        if let Some(v_start) = rest.find("<Value>") {
            let val_rest = &rest[v_start + 7..];
            if let Some(v_end) = val_rest.find("</Value>") {
                return val_rest[..v_end].trim().to_string();
            }
        }
    }

    // Direct tag fallback: <Key>Value</Key>
    let direct_tag = format!("<{}>", key);
    let direct_end = format!("</{}>", key);
    if let Some(s) = block.find(&direct_tag) {
        let rest = &block[s + direct_tag.len()..];
        if let Some(e) = rest.find(&direct_end) {
            return rest[..e].trim().to_string();
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_matrix_parsing() {
        let csv = "title,username,password\n\"GitHub, Inc.\",user@example.com,\"secret\"\"pass\"";
        let matrix = parse_csv_matrix(csv);
        assert_eq!(matrix.len(), 2);
        assert_eq!(matrix[0], vec!["title", "username", "password"]);
        assert_eq!(matrix[1][0], "GitHub, Inc.");
        assert_eq!(matrix[1][1], "user@example.com");
        assert_eq!(matrix[1][2], "secret\"pass");
    }

    #[test]
    fn test_bitwarden_json_parsing() {
        let json_data = r#"{
            "items": [
                {
                    "name": "Google Account",
                    "notes": "Personal email account",
                    "type": 1,
                    "login": {
                        "username": "alice@gmail.com",
                        "password": "supersecretpassword123",
                        "totp": "otpauth://totp/Google?secret=JBSWY3DPEHPK3PXP",
                        "uris": [
                            {"uri": "https://accounts.google.com"},
                            {"uri": "https://myaccount.google.com"}
                        ]
                    },
                    "fields": [
                        {"name": "Recovery Key", "value": "1234-5678", "type": 1}
                    ]
                }
            ]
        }"#;

        let res = Importer::parse_str(json_data, ImportFormat::BitwardenJson).unwrap();
        assert_eq!(res.total_found, 1);
        let entry = &res.entries[0];
        assert_eq!(entry.title, "Google Account");
        assert_eq!(entry.username, "alice@gmail.com");
        assert_eq!(entry.password, "supersecretpassword123");
        assert_eq!(entry.url, "https://accounts.google.com");
        assert_eq!(entry.totp_secret, Some("JBSWY3DPEHPK3PXP".to_string()));
        assert_eq!(entry.custom_fields.len(), 2); // 1 alt URI + 1 custom field
        assert_eq!(entry.custom_fields[1].name, "Recovery Key");
        assert_eq!(entry.custom_fields[1].value, "1234-5678");
    }

    #[test]
    fn test_totp_uri_cleaner() {
        assert_eq!(clean_totp_secret("otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&issuer=Test"), Some("JBSWY3DPEHPK3PXP".to_string()));
        assert_eq!(clean_totp_secret("jbsw y3dp-ehpk 3pxp"), Some("JBSWY3DPEHPK3PXP".to_string()));
    }
}
