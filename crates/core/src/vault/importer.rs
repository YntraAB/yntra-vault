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
#[derive(Default)]
pub enum ImportFormat {
    #[default]
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


/// Upper bounds on import data to prevent memory exhaustion and DoS attacks.
pub const MAX_IMPORT_PAYLOAD_BYTES: usize = 50 * 1024 * 1024; // 50 MB
pub const MAX_IMPORT_ENTRIES: usize = 50_000;

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
    parse_csv_checked(content).unwrap_or_default()
}

fn parse_csv_checked(content: &str) -> crate::Result<Vec<Vec<String>>> {
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
                '"' if current_field.is_empty() => in_quotes = true,
                '"' => current_field.push('"'),
                ',' => {
                    current_row.push(std::mem::take(&mut current_field));
                    current_field = String::new();
                }
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') { chars.next(); }
                    current_row.push(std::mem::take(&mut current_field));
                    if !current_row.iter().all(|f| f.is_empty()) {
                        rows.push(current_row);
                    }
                    current_row = Vec::new();
                    current_field = String::new();
                }
                _ => current_field.push(c),
            }
        }
    }

    if in_quotes {
        return Err(crate::VaultError::InvalidFormat("Unclosed quoted CSV field".into()));
    }
    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        if !current_row.iter().all(|f| f.is_empty()) {
            rows.push(current_row);
        }
    }

    Ok(rows)
}

/// Preserve URI parameters (algorithm, digits, period and issuer) for code generation.
pub fn clean_totp_secret(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("otpauth://") || trimmed.starts_with("steam://") {
        return Some(trimmed.to_owned());
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
        let metadata = std::fs::metadata(path)
            .map_err(|e| crate::VaultError::InvalidFormat(format!("Failed to read import file metadata: {}", e)))?;
        if metadata.len() > MAX_IMPORT_PAYLOAD_BYTES as u64 {
            return Err(crate::VaultError::InvalidFormat(format!(
                "Import file exceeds maximum allowed size ({} MB)",
                MAX_IMPORT_PAYLOAD_BYTES / (1024 * 1024)
            )));
        }

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
        if content.len() > MAX_IMPORT_PAYLOAD_BYTES {
            return Err(crate::VaultError::InvalidFormat(format!(
                "Import payload exceeds maximum allowed size ({} MB)",
                MAX_IMPORT_PAYLOAD_BYTES / (1024 * 1024)
            )));
        }

        let trimmed = content.trim_start_matches('\u{feff}');
        if trimmed.trim().is_empty() {
            return Err(crate::VaultError::InvalidFormat("The imported file is empty.".to_string()));
        }

        let auto_detected = Self::detect_format(trimmed);

        // A recognized file structure takes precedence over the selected brand.
        let final_format = if auto_detected != ImportFormat::GenericCsv
            || requested_format == ImportFormat::AutoDetect {
            auto_detected
        } else {
            requested_format
        };
        let is_mismatch = requested_format != ImportFormat::AutoDetect
            && requested_format != final_format;
        let suggested_brand = is_mismatch.then(|| Self::format_to_brand_name(final_format).to_owned());
        let entries = Self::execute_parse(trimmed, final_format)?;

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
        let entries = match format {
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

        if entries.len() > MAX_IMPORT_ENTRIES {
            return Err(crate::VaultError::InvalidFormat(format!("Import exceeds the limit of {MAX_IMPORT_ENTRIES} entries. Split the export into smaller files.")));
        }
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
        let content = content.trim_start();
        if content.starts_with('{') {
            if serde_json::from_str::<serde_json::Value>(content).ok()
                .is_some_and(|json| json.get("vaults").is_some()) {
                return ImportFormat::ProtonPassJson;
            }
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
            let h_lower: Vec<String> = header.iter().map(|s| s.trim().to_lowercase()).collect();

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

        let items = json.get("items").and_then(|i| i.as_array())
            .ok_or_else(|| crate::VaultError::InvalidFormat("Missing Bitwarden items array".into()))?;
        {
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

                for key in ["card", "identity", "sshKey"] {
                    if let Some(fields) = item.get(key).and_then(|v| v.as_object()) {
                        for (name, value) in fields {
                            if !value.is_null() {
                                custom_fields.push(CustomField { id: Uuid::new_v4(), name: name.clone(),
                                    value: value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()),
                                    field_type: FieldType::Password, sensitive: true });
                            }
                        }
                    }
                }
                let tags = json.get("folders").and_then(|v| v.as_array()).into_iter().flatten()
                    .filter(|folder| item.get("folderId").is_some_and(|id| !id.is_null() && folder.get("id") == Some(id)))
                    .filter_map(|folder| folder.get("name").and_then(|v| v.as_str()))
                    .map(str::to_owned).collect();
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
                    tags,
                    is_duplicate: false,
                    duplicate_reason: None,
                });
            }
        }

        Ok(result)
    }

    /// Bitwarden CSV parser
    fn parse_bitwarden_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_checked(content)?;
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

            let mut tags = Vec::new();
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
        let matrix = parse_csv_checked(content)?;
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["title", "name"]);
            let username = idx.get(row, &["username", "user", "login", "login name", "email"]);
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
                tags: idx.get(row, &["tags", "vault", "folder"]).split(';').map(str::trim).filter(|tag| !tag.is_empty()).map(str::to_owned).collect(),
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// KeePass / KeePassXC CSV parser
    fn parse_keepass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_checked(content)?;
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["title"]);
            let username = idx.get(row, &["username", "user_name", "login name"]);
            let password = idx.get(row, &["password"]);
            let url = idx.get(row, &["url"]);
            let notes = idx.get(row, &["notes", "comment"]);
            let group = idx.get(row, &["group"]);
            let totp_raw = idx.get(row, &["totp"]);

            let email = if username.contains('@') { username.clone() } else { String::new() };
            let totp_secret = clean_totp_secret(&totp_raw);

            let mut tags = Vec::new();
            if !group.is_empty() {
                tags.push(group);
            }
            for tag in idx.get(row, &["tags"]).split(';').map(str::trim).filter(|tag| !tag.is_empty()) {
                if !tags.iter().any(|existing| existing == tag) { tags.push(tag.to_owned()); }
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

    /// Parse structural XML boundaries; history and empty values never leak into live fields.
    fn parse_keepass_xml(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 2_000_000, ..Default::default() };
        let document = roxmltree::Document::parse_with_options(content, options).map_err(|error| {
            let position = error.pos();
            crate::VaultError::InvalidFormat(format!("Invalid KeePass XML at line {}, column {}", position.row, position.col))
        })?;
        if !document.root_element().has_tag_name("KeePassFile") {
            return Err(crate::VaultError::InvalidFormat("Expected a KeePass XML export".into()));
        }
        let mut result = Vec::new();
        for entry in document.descendants().filter(|n| n.has_tag_name("Entry")) {
            if entry.ancestors().skip(1).any(|n| n.has_tag_name("History") || n.has_tag_name("Entry")) { continue; }
            if result.len() >= MAX_IMPORT_ENTRIES {
                return Err(crate::VaultError::InvalidFormat(format!("Import exceeds the limit of {MAX_IMPORT_ENTRIES} entries")));
            }
            let mut fields = std::collections::BTreeMap::new();
            for field in entry.children().filter(|n| n.has_tag_name("String")) {
                let key = field.children().find(|n| n.has_tag_name("Key")).map(xml_text).transpose()?.unwrap_or_default();
                let value = field.children().find(|n| n.has_tag_name("Value"));
                if value.is_some_and(|n| n.attribute("Protected").is_some_and(|v| v.eq_ignore_ascii_case("true"))) {
                    return Err(crate::VaultError::InvalidFormat("KeePass XML still contains encrypted values. Export a decrypted XML file from KeePassXC first".into()));
                }
                let text = value.map(xml_text).transpose()?.unwrap_or_default();
                if fields.insert(key, text).is_some() {
                    return Err(crate::VaultError::InvalidFormat("Duplicate field in KeePass XML entry".into()));
                }
            }
            let title = fields.remove("Title").unwrap_or_default();
            let username = fields.remove("UserName").unwrap_or_default();
            let password = fields.remove("Password").unwrap_or_default();
            let url = fields.remove("URL").unwrap_or_default();
            let notes = fields.remove("Notes").unwrap_or_default();
            let totp_raw = fields.remove("TimeOtp-Secret-Base32").or_else(||fields.remove("otp")).unwrap_or_default();
            let totp_secret = clean_totp_secret(&totp_raw);
            let mut tags = Vec::new();
            if let Some(group) = entry.ancestors().find(|n| n.has_tag_name("Group")) {
                if let Some(name) = group.children().find(|n| n.has_tag_name("Name")) {
                    let name=xml_text(name)?.trim().to_owned();
                    if !name.is_empty() && name != "Root" && !tags.contains(&name) { tags.push(name); }
                }
            }
            if let Some(node) = entry.children().find(|n| n.has_tag_name("Tags")) {
                for tag in xml_text(node)?.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    if !tags.iter().any(|existing| existing == tag) { tags.push(tag.to_owned()); }
                }
            }
            let email = if username.contains('@') { username.clone() } else { String::new() };
            let custom_fields = fields.into_iter().map(|(name, value)| CustomField {
                id: Uuid::new_v4(), name, value, field_type: FieldType::Password, sensitive: true,
            }).collect();
            result.push(ParsedImportEntry { title, username, password, url, email, notes, totp_secret,
                custom_fields, entry_type: EntryType::Login, tags, is_duplicate: false, duplicate_reason: None });
        }
        Ok(result)
    }

    /// Chrome / Edge / Firefox CSV parser
    fn parse_chrome_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_checked(content)?;
        if matrix.len() < 2 {
            return Ok(Vec::new());
        }

        let header = &matrix[0];
        let idx = HeaderIndex::new(header);

        let mut result = Vec::new();
        for row in matrix.iter().skip(1) {
            let title = idx.get(row, &["name", "title"]);
            let url = idx.get(row, &["url", "url_href", "hostname", "website"]);
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
                tags: idx.get(row, &["tags", "vault", "folder"]).split(';').map(str::trim).filter(|tag| !tag.is_empty()).map(str::to_owned).collect(),
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// LastPass CSV parser
    fn parse_lastpass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_checked(content)?;
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

            let mut tags = Vec::new();
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
        let matrix = parse_csv_checked(content)?;
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
                tags: idx.get(row, &["tags", "vault", "folder"]).split(';').map(str::trim).filter(|tag| !tag.is_empty()).map(str::to_owned).collect(),
                is_duplicate: false,
                duplicate_reason: None,
            });
        }

        Ok(result)
    }

    /// Proton Pass JSON parser
    fn parse_protonpass_json(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let json: serde_json::Value = serde_json::from_str(content)
            .map_err(|_| crate::VaultError::InvalidFormat("Invalid Proton Pass JSON".into()))?;
        let vaults: Vec<&serde_json::Value> = match json.get("vaults") {
            Some(serde_json::Value::Object(vaults)) => vaults.values().collect(),
            Some(serde_json::Value::Array(vaults)) => vaults.iter().collect(),
            _ => return Err(crate::VaultError::InvalidFormat("Missing Proton Pass vaults".into())),
        };
        let mut result = Vec::new();
        for vault in vaults {
            let items = vault.get("items").and_then(|v| v.as_array())
                .ok_or_else(|| crate::VaultError::InvalidFormat("Missing Proton Pass items".into()))?;
            for item in items {
                let data = item.get("data").ok_or_else(|| crate::VaultError::InvalidFormat("Missing Proton Pass item data".into()))?;
                let metadata = data.get("metadata").unwrap_or(item);
                let content = data.get("content").unwrap_or(data);
                let text = |object: &serde_json::Value, keys: &[&str]| -> String {
                    keys.iter().filter_map(|key| object.get(key).and_then(|v| v.as_str())).find(|v| !v.is_empty()).unwrap_or("").to_owned()
                };
                let email = text(content, &["itemEmail", "email"]);
                let mut username = text(content, &["itemUsername", "username"]);
                if username.is_empty() { username = email.clone(); }
                let urls = content.get("urls").and_then(|v| v.as_array());
                let url = urls.and_then(|urls| urls.first()).and_then(|v| v.as_str().or_else(|| v.get("url").and_then(|v| v.as_str()))).unwrap_or("").to_owned();
                let entry_type = match data.get("type").and_then(|v| v.as_str()) {
                    Some("note") => EntryType::SecureNote,
                    Some("creditCard") => EntryType::CreditCard,
                    Some("identity") => EntryType::Identity,
                    _ => EntryType::Login,
                };
                // Retain extended fields and unsupported item data rather than silently discarding it.
                let custom_fields = vec![CustomField { id: Uuid::new_v4(), name: "Proton Pass data".into(),
                    value: data.to_string(), field_type: FieldType::Password, sensitive: true }];
                let tags = vault.get("name").and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| vec![v.to_owned()]).unwrap_or_default();
                result.push(ParsedImportEntry {
                    title: text(metadata, &["name", "title"]), username, email, url,
                    password: text(content, &["password"]), notes: text(metadata, &["note", "notes"]),
                    totp_secret: clean_totp_secret(&text(content, &["totpUri", "totp"])),
                    custom_fields, entry_type, tags, is_duplicate: false, duplicate_reason: None,
                });
            }
        }
        Ok(result)
    }

    /// Proton Pass CSV parser
    fn parse_protonpass_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        Self::parse_generic_csv(content)
    }

    /// Generic CSV parser with explicit column aliases and headerless positional fallback
    fn parse_generic_csv(content: &str) -> crate::Result<Vec<ParsedImportEntry>> {
        let matrix = parse_csv_checked(content)?;
        if matrix.is_empty() {
            return Ok(Vec::new());
        }

        // Do not discard the first record merely because more than one row exists.
        let has_header = matrix[0].iter().any(|cell| matches!(
            cell.trim().to_ascii_lowercase().as_str(),
            "title" | "name" | "username" | "password" | "url" | "website" | "login" | "email"
        ));
        let start_row = if has_header { 1 } else { 0 };

        let idx = if has_header {
            HeaderIndex::new(&matrix[0])
        } else {
            HeaderIndex::new(&[])
        };

        let mut result = Vec::new();
        for row in matrix.iter().skip(start_row) {
            let mut title = idx.get(row, &["title", "name", "item", "service", "account"]);
            let mut username = idx.get(row, &["username", "user", "login", "login name", "email"]);
            let mut password = idx.get(row, &["password", "pass", "secret"]);
            let mut url = idx.get(row, &["url", "website", "link", "uri", "hostname"]);
            let mut notes = idx.get(row, &["notes", "note", "comment", "description"]);
            let totp_raw = idx.get(row, &["totp", "otp", "totp_secret"]);

            // Positional index fallback if headers were unmapped
            if !has_header {
                if !row.is_empty() { title = row[0].to_string(); }
                if row.len() > 1 { username = row[1].to_string(); }
                if row.len() > 2 { password = row[2].to_string(); }
                if row.len() > 3 { url = row[3].to_string(); }
                if row.len() > 4 { notes = row[4].to_string(); }
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
                tags: idx.get(row, &["tags", "vault", "folder"]).split(';').map(str::trim).filter(|tag| !tag.is_empty()).map(str::to_owned).collect(),
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
            let clean = h.trim_start_matches('\u{feff}').trim().to_lowercase().replace([' ', '_', '-'], "");
            map.insert(clean, i);
        }
        Self { map }
    }

    fn get(&self, row: &[String], candidates: &[&str]) -> String {
        // 1. Try exact matches first
        for cand in candidates {
            let clean = cand.replace([' ', '_', '-'], "");
            if let Some(&idx) = self.map.get(&clean)
                && idx < row.len() {
                    let val = &row[idx];
                    if !val.is_empty() {
                        return val.to_string();
                    }
                }
        }

        String::new()
    }
}

/// The parser resolves XML entities once; preserve whitespace inside credentials and notes.
fn xml_text(node: roxmltree::Node<'_, '_>) -> crate::Result<String> {
    if node.children().any(|child| child.is_element()) {
        return Err(crate::VaultError::InvalidFormat("Unexpected nested element in KeePass field".into()));
    }
    Ok(node.children().filter(|n| n.is_text()).filter_map(|n| n.text()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proton_object_vaults_and_bitwarden_folders_preserve_data() {
        let proton = r#"{"vaults":{"v1":{"name":"Personal","items":[{"data":{"type":"login","metadata":{"name":"GitHub","note":"fixture note"},"content":{"itemUsername":"alice","itemEmail":"alice@example.test","password":" fixture ","urls":["https://github.com"],"totpUri":"otpauth://totp/GitHub?secret=JBSWY3DPEHPK3PXP&digits=8"},"extraFields":[{"fieldName":"backup","data":{"content":"fixture"}}]}}]}}}"#;
        let preview = Importer::parse_str(proton, ImportFormat::AutoDetect).unwrap();
        assert_eq!(preview.detected_format_key, "protonpass_json");
        assert_eq!(preview.entries.len(), 1);
        assert_eq!(preview.entries[0].title, "GitHub");
        assert_eq!(preview.entries[0].password, " fixture ");
        assert_eq!(preview.entries[0].tags, vec!["Personal"]);
        assert!(preview.entries[0].totp_secret.as_ref().unwrap().contains("digits=8"));
        assert!(preview.entries[0].custom_fields[0].value.contains("extraFields"));
        let bitwarden = r#"{"folders":[{"id":"work","name":"Work"}],"items":[{"name":"Card","folderId":"work","type":3,"card":{"number":"fixture","code":"123"}}]}"#;
        let preview = Importer::parse_str(bitwarden, ImportFormat::AutoDetect).unwrap();
        assert_eq!(preview.entries[0].tags, vec!["Work"]);
        assert_eq!(preview.entries[0].custom_fields.len(), 2);
    }

    #[test]
    fn csv_preserves_credentials_and_does_not_use_partial_header_matches() {
        let preview = Importer::parse_str("\u{feff}Title,Username,Password,One-time password\r\nGitHub, User ,\" secret \",OTP", ImportFormat::OnePasswordCsv).unwrap();
        assert_eq!(preview.entries[0].username, " User ");
        assert_eq!(preview.entries[0].password, " secret ");
        let empty = Importer::parse_str("Title,Username,Password,One-time password\nGitHub,,,OTP", ImportFormat::OnePasswordCsv).unwrap();
        assert_eq!(empty.entries[0].password, "");
    }

    #[test]
    fn supported_csv_formats_preserve_both_accounts_and_secret_whitespace() {
        for (format, header) in [
            (ImportFormat::BitwardenCsv, "name,login_username,login_password,login_uri"),
            (ImportFormat::OnePasswordCsv, "Title,Username,Password,URL"),
            (ImportFormat::KeepassCsv, "Title,User Name,Password,URL"),
            (ImportFormat::ChromeCsv, "name,username,password,url"),
            (ImportFormat::LastPassCsv, "name,username,password,url"),
            (ImportFormat::DashlaneCsv, "title,login,password,url"),
            (ImportFormat::ProtonPassCsv, "name,username,password,url"),
            (ImportFormat::GenericCsv, "title,username,password,url"),
        ] {
            let csv = format!("{header}\r\nGitHub,alice,\" first \",https://github.com\r\nGitHub,bob,\" second \",https://github.com");
            let entries = Importer::parse_str(&csv, format).unwrap().entries;
            assert_eq!(entries.len(), 2, "{format:?}");
            assert_eq!(entries[0].username, "alice", "{format:?}");
            assert_eq!(entries[1].password, " second ", "{format:?}");
        }
    }

    #[test]
    fn headerless_csv_keeps_first_entry_and_header_only_is_empty() {
        let preview = Importer::parse_str("GitHub,alice,one\nGitLab,bob,two", ImportFormat::GenericCsv).unwrap();
        assert_eq!(preview.total_found, 2);
        assert_eq!(preview.entries[0].title, "GitHub");
        assert_eq!(Importer::parse_str("title,username,password", ImportFormat::GenericCsv).unwrap().total_found, 0);
        let firefox = Importer::parse_str("hostname,username,password\nhttps://github.com,alice,fixture", ImportFormat::AutoDetect).unwrap();
        assert_eq!(firefox.entries[0].url, "https://github.com");
        let legacy = Importer::parse_str("Account,Login Name,Password,Web Site\nGitHub,alice,fixture,https://github.com", ImportFormat::AutoDetect).unwrap();
        assert_eq!(legacy.entries[0].username, "alice");
        assert_eq!(legacy.entries[0].url, "https://github.com");
    }

    #[test]
    fn parser_errors_are_not_reported_as_successful_empty_imports() {
        assert!(Importer::parse_str("{broken", ImportFormat::AutoDetect).is_err());
        assert!(Importer::parse_str("title,username,password\nGitHub,alice,\"unfinished", ImportFormat::AutoDetect).is_err());
        assert!(Importer::parse_str(r#"{"encrypted":true}"#, ImportFormat::AutoDetect).is_err());
        let preview = Importer::parse_str("<KeePassFile><Entry><String><Key>Token</Key><Value>fixture</Value></String></Entry></KeePassFile>", ImportFormat::BitwardenJson).unwrap();
        assert!(preview.is_format_mismatch);
        assert_eq!(preview.total_found, 1);
        assert_eq!(preview.entries[0].custom_fields[0].value, "fixture");
        assert!(preview.entries[0].custom_fields[0].sensitive);
    }

    #[test]
    fn import_limit_rejects_instead_of_silently_truncating() {
        let csv = format!("title,username,password\n{}", "GitHub,user,fixture\n".repeat(MAX_IMPORT_ENTRIES + 1));
        assert!(Importer::parse_str(&csv, ImportFormat::AutoDetect).is_err());
    }

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
        assert_eq!(entry.totp_secret, Some("otpauth://totp/Google?secret=JBSWY3DPEHPK3PXP".to_string()));
        assert_eq!(entry.custom_fields.len(), 2); // 1 alt URI + 1 custom field
        assert_eq!(entry.custom_fields[1].name, "Recovery Key");
        assert_eq!(entry.custom_fields[1].value, "1234-5678");
    }

    #[test]
    fn test_totp_uri_cleaner() {
        assert_eq!(clean_totp_secret("otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&issuer=Test"), Some("otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&issuer=Test".to_string()));
        assert_eq!(clean_totp_secret("jbsw y3dp-ehpk 3pxp"), Some("JBSWY3DPEHPK3PXP".to_string()));
    }

    #[test]
    fn keepass_empty_fields_cannot_capture_the_next_field() {
        for empty in ["<Value />", "<Value/>", "<Value ProtectInMemory=\"True\" />", "<Value></Value>", ""] {
            let xml = format!(r#"<KeePassFile><Root><Group><Name>Apps</Name><Entry>
                <String><Key>Title</Key><Value>Example</Value></String>
                <String><Key>Notes</Key>{empty}</String>
                <String><Key>Password</Key><Value ProtectInMemory="True">  test&amp;lt;&lt;value&gt;  </Value></String>
                <String><Key>URL</Key>{empty}</String>
                <String><Key>UserName</Key><Value>Example user</Value></String>
            </Entry></Group></Root></KeePassFile>"#);
            let preview=Importer::parse_str(&xml,ImportFormat::KeepassXml).unwrap();
            assert_eq!(preview.entries.len(),1);
            let entry=&preview.entries[0];
            assert_eq!(entry.notes,"");assert_eq!(entry.url,"");
            assert_eq!(entry.username,"Example user");
            assert_eq!(entry.password,"  test&lt;<value>  ");
        }
    }

    #[test]
    fn import_tags_come_only_from_the_users_export() {
        let keepass = Importer::parse_str("Group,Title,Username,Password,Tags\nPersonal,Test,me,secret,Work & Life;KeePass", ImportFormat::KeepassCsv).unwrap();
        assert_eq!(keepass.entries[0].tags, vec!["Personal", "Work & Life", "KeePass"]);
        let untagged = Importer::parse_str("Title,Username,Password\nTest,me,secret", ImportFormat::KeepassCsv).unwrap();
        assert!(untagged.entries[0].tags.is_empty());
        let generic = Importer::parse_str("title,username,password\nTest,me,secret", ImportFormat::GenericCsv).unwrap();
        assert!(generic.entries[0].tags.is_empty());
        let bitwarden = Importer::parse_str(r#"{"items":[{"name":"Test","type":1,"login":{"username":"me","password":"secret"}}]}"#, ImportFormat::BitwardenJson).unwrap();
        assert!(bitwarden.entries[0].tags.is_empty());
    }

    #[test]
    fn keepass_groups_entities_cdata_and_history_stay_in_their_scope() {
        let xml=r#"<KeePassFile><Root><Group><Name>Parent &amp; One</Name>
          <Group><Name>Child &amp; Two</Name><Entry><String><Key>Title</Key><Value>Child</Value></String>
          <Tags>Work &amp; Life;Numeric &#38; Tag;Literal &amp;amp;</Tags>
          <String><Key>Notes</Key><Value><![CDATA[ <note> & literal ]]></Value></String>
          <History><Entry><String><Key>Title</Key><Value>Old child</Value></String><String><Key>URL</Key><Value>https://old.invalid</Value></String></Entry></History>
          </Entry></Group>
          <Entry><String><Key>Title</Key><Value>Parent</Value></String><String><Key>Password</Key><Value>&#x20;test&#32;</Value></String></Entry>
          </Group></Root></KeePassFile>"#;
        let preview=Importer::parse_str(xml,ImportFormat::KeepassXml).unwrap();
        assert_eq!(preview.entries.len(),2);
        assert_eq!(preview.entries[0].tags,vec!["Child & Two","Work & Life","Numeric & Tag","Literal &amp"]);
        assert_eq!(preview.entries[0].notes," <note> & literal ");
        assert_eq!(preview.entries[0].url,"");
        assert_eq!(preview.entries[1].tags,vec!["Parent & One"]);
        assert_eq!(preview.entries[1].password," test ");
    }

    #[test]
    fn keepass_invalid_or_still_encrypted_xml_reports_an_error() {
        for xml in [
            "<KeePassFile><Root></KeePassFile>",
            "<!DOCTYPE KeePassFile [<!ENTITY unsafe SYSTEM 'file:///test'>]><KeePassFile>&unsafe;</KeePassFile>",
            "<KeePassFile><Entry><String><Key>Password</Key><Value Protected='True'>ciphertext</Value></String></Entry></KeePassFile>",
            "<KeePassFile><Entry><String><Key>Notes</Key><Value><String>nested</String></Value></String></Entry></KeePassFile>",
        ] { assert!(Importer::parse_str(xml,ImportFormat::KeepassXml).is_err()); }
    }

    #[test]
    fn test_keepass_xml_protected_in_memory_and_entity_unescape() {
        let xml_data = r#"<KeePassFile>
            <Root>
                <Group>
                    <Entry>
                        <String>
                            <Key>Title</Key>
                            <Value>Banking &amp; Finance</Value>
                        </String>
                        <String>
                            <Key>UserName</Key>
                            <Value>john_doe</Value>
                        </String>
                        <String>
                            <Key>Password</Key>
                            <Value ProtectInMemory="True">P@ss&amp;w0rd&lt;123&gt;</Value>
                        </String>
                        <String>
                            <Key>URL</Key>
                            <Value>https://bank.example.com</Value>
                        </String>
                    </Entry>
                </Group>
            </Root>
        </KeePassFile>"#;

        let res = Importer::parse_str(xml_data, ImportFormat::KeepassXml).unwrap();
        assert_eq!(res.total_found, 1);
        let entry = &res.entries[0];
        assert_eq!(entry.title, "Banking & Finance");
        assert_eq!(entry.username, "john_doe");
        assert_eq!(entry.password, "P@ss&w0rd<123>");
        assert_eq!(entry.url, "https://bank.example.com");
    }
}
