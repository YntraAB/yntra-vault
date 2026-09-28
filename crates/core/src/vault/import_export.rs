//! Import and export operations for VaultManager.
//!
//! Handles duplicate detection, bulk importing with conflict strategies,
//! and CSV/JSON vault exports.

use chrono::Utc;
use std::path::Path;
use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::entry::{NewEntry, UpdateEntry};
use crate::vault::importer::{DuplicateStrategy, ParsedImportEntry};
use crate::vault::manager::VaultManager;
use crate::vault::types::{BreachStatus, Entry, FieldScope, Tag};

// Empty usernames are identities, never wildcards. Keep case-sensitive logins distinct.
fn import_identity_matches(entry: &Entry, item: &ParsedImportEntry) -> bool {
    !item.title.trim().is_empty()
        && entry.title.trim().eq_ignore_ascii_case(item.title.trim())
        && entry.username == item.username
        && entry.url.trim() == item.url.trim()
        && entry.entry_type == item.entry_type
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::vault::importer::{Importer, ImportFormat};

    #[test]
    fn plaintext_export_rejects_source_and_hardlink_without_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.vdb");
        let alias = directory.path().join("alias.vdb");
        let manager = VaultManager::create("Export", "synthetic password 2026", &path).unwrap();
        std::fs::hard_link(&path, &alias).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(manager.export_csv(&path).is_err());
        assert!(manager.export_json(&alias).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read(&alias).unwrap(), before);
        let output = directory.path().join("export.json");
        manager.export_json(&output).unwrap();
        assert_eq!(std::fs::read_to_string(output).unwrap(), "[]");
    }

    #[test]
    fn records_in_the_same_export_are_not_silently_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let mut manager = VaultManager::create("Import", "fixture-password", &dir.path().join("import.vdb")).unwrap();
        let mut entries = Importer::parse_str("title,username,password\nGitHub,alice,first\nGitHub,alice,second", ImportFormat::GenericCsv).unwrap().entries;
        assert_eq!(manager.check_import_duplicates(&mut entries), 0);
        assert_eq!(manager.bulk_import_entries(entries, DuplicateStrategy::Skip).unwrap(), 2);
    }

    #[test]
    fn github_accounts_remain_distinct_and_overwrite_registers_tags() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("import.vdb");
        let mut manager = VaultManager::create("Import", "fixture-password", &path).unwrap();
        let csv = "title,username,password,url\nGitHub,,one,https://github.com\nGitHub,alice,two,https://github.com\nGitHub,Alice,three,https://github.com\nGitHub,alice,four,https://enterprise.invalid";
        let entries = Importer::parse_str(csv, ImportFormat::GenericCsv).unwrap().entries;
        assert_eq!(manager.bulk_import_entries(entries.clone(), DuplicateStrategy::Skip).unwrap(), 4);
        let mut preview = entries.clone();
        assert_eq!(manager.check_import_duplicates(&mut preview), 4);
        let mut changed = entries[1].clone();
        changed.password = "updated-fixture".into();
        changed.tags = vec![" Work ".into()];
        assert_eq!(manager.bulk_import_entries(vec![changed], DuplicateStrategy::Overwrite).unwrap(), 1);
        assert!(manager.data.tags.iter().any(|t| t.name == "Work"));
        assert_eq!(manager.data.entries[1].tags, vec!["Work"]);
        let reopened = VaultManager::open(&path, "fixture-password").unwrap();
        assert_eq!(reopened.data.entries.len(), 4);
        assert_eq!(reopened.get_entry(reopened.data.entries[1].id).unwrap().password, "updated-fixture");
    }

    #[test]
    fn failed_bulk_save_rolls_back_new_entries_overwrites_and_tags() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("import.vdb");
        let mut manager = VaultManager::create("Import", "fixture-password", &path).unwrap();
        let mut entries = Importer::parse_str("title,username,password\nGitHub,alice,old", ImportFormat::GenericCsv).unwrap().entries;
        manager.bulk_import_entries(entries.clone(), DuplicateStrategy::KeepBoth).unwrap();
        let original = std::fs::read(&path).unwrap();
        let id = manager.data.entries[0].id;
        entries[0].password = "changed".into();
        entries[0].tags = vec!["New tag".into()];
        let mut new = entries[0].clone(); new.title = "GitLab".into(); entries.push(new);
        manager.path = dir.path().to_owned(); // A directory cannot be replaced by a vault file.
        assert!(manager.bulk_import_entries(entries, DuplicateStrategy::Overwrite).is_err());
        assert_eq!(manager.data.entries.len(), 1);
        assert_eq!(manager.get_entry(id).unwrap().password, "old");
        assert!(!manager.data.tags.iter().any(|t| t.name == "New tag"));
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

impl VaultManager {
    /// Checks an array of ParsedImportEntry against current vault entries for duplicates.
    pub fn check_import_duplicates(&self, entries: &mut [ParsedImportEntry]) -> usize {
        let mut dup_count = 0;
        for item in entries.iter_mut() {
            item.is_duplicate = false;
            item.duplicate_reason = None;
            let match_found = self.data.entries.iter().any(|e| import_identity_matches(e, item));

            if match_found {
                item.is_duplicate = true;
                item.duplicate_reason = Some(format!("An entry titled '{}' already exists.", item.title));
                dup_count += 1;
            }
        }
        dup_count
    }

    /// Bulk imports parsed entries into the vault using the chosen DuplicateStrategy.
    pub fn bulk_import_entries(
        &mut self,
        entries: Vec<ParsedImportEntry>,
        strategy: DuplicateStrategy,
    ) -> crate::Result<usize> {
        if entries.len() > super::importer::MAX_IMPORT_ENTRIES {
            return Err(VaultError::InvalidFormat("Too many import entries".into()));
        }
        let previous = self.data.clone();
        let result = self.bulk_import_entries_inner(entries, strategy);
        if result.is_err() {
            self.data = previous;
            self.rebuild_search_index();
        }
        result
    }

    fn bulk_import_entries_inner(&mut self, entries: Vec<ParsedImportEntry>, strategy: DuplicateStrategy) -> crate::Result<usize> {
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry_key = keys.entry_key.clone();
        let mut count = 0;
        // Match the same pre-import vault that the preview inspected.
        let existing_count = self.data.entries.len();

        let tag_colors = ["#3b82f6", "#10b981", "#8b5cf6", "#f59e0b", "#ec4899", "#06b6d4"];
        let mut color_idx = 0;

        for mut item in entries {
            item.tags = item.tags.into_iter().map(|tag| tag.trim().to_owned()).filter(|tag| !tag.is_empty()).collect();
            let existing_id = self.data.entries.iter().take(existing_count)
                .find(|e| import_identity_matches(e, &item)).map(|e| e.id);
            if existing_id.is_some() && matches!(strategy, DuplicateStrategy::Skip) { continue; }

            for tag_name in &item.tags {
                let tag_name = tag_name.trim();
                if !tag_name.is_empty() && !self.data.tags.iter().any(|t| t.name == tag_name) {
                    self.data.tags.push(Tag { id: Uuid::new_v4(), name: tag_name.into(),
                        color: tag_colors[color_idx % tag_colors.len()].into(), icon: "tag".into() });
                    color_idx += 1;
                }
            }

            if let Some(target_id) = existing_id {
                match strategy {
                    DuplicateStrategy::Skip => continue,
                    DuplicateStrategy::Overwrite => {
                        let update = UpdateEntry {
                            title: Some(item.title),
                            username: Some(item.username),
                            password: if !item.password.is_empty() {
                                Some(item.password)
                            } else {
                                None
                            },
                            url: if !item.url.is_empty() {
                                Some(item.url)
                            } else {
                                None
                            },
                            email: if !item.email.is_empty() {
                                Some(item.email)
                            } else {
                                None
                            },
                            notes: if !item.notes.is_empty() {
                                Some(item.notes)
                            } else {
                                None
                            },
                            totp_secret: item.totp_secret,
                            tags: if !item.tags.is_empty() {
                                Some(item.tags)
                            } else {
                                None
                            },
                            custom_fields: if !item.custom_fields.is_empty() {
                                Some(item.custom_fields.clone())
                            } else {
                                None
                            },
                            ..Default::default()
                        };
                        self.update_entry_inner(target_id, update, false)?;
                        count += 1;
                        continue;
                    }
                    DuplicateStrategy::KeepBoth => {
                        // Fallthrough to add as new entry
                    }
                }
            }

            let new_entry = NewEntry {
                title: item.title,
                username: item.username,
                password: item.password,
                url: item.url,
                email: item.email,
                notes: item.notes,
                tags: item.tags,
                totp_secret: item.totp_secret,
                custom_fields: item.custom_fields,
                entry_type: Some(item.entry_type.clone()),
                generate_passkey: None,
                attachments: None,
            };

            let now = Utc::now();
            let id = Uuid::new_v4();

            let encrypted_password = Self::encrypt_entry_field(
                new_entry.password.as_bytes(),
                &entry_key,
                &id,
                FieldScope::Password,
            )?;

            let encrypted_totp = if let Some(ref secret) = new_entry.totp_secret {
                Some(Self::encrypt_entry_field(
                    secret.as_bytes(),
                    &entry_key,
                    &id,
                    FieldScope::Totp,
                )?)
            } else {
                None
            };

            let entry = Entry {
                id,
                title: new_entry.title.clone(),
                username: new_entry.username.clone(),
                encrypted_password,
                url: new_entry.url.clone(),
                email: new_entry.email.clone(),
                notes: new_entry.notes.clone(),
                tags: new_entry.tags.clone(),
                favorite: false,
                pinned: false,
                encrypted_totp_secret: encrypted_totp,
                custom_fields: new_entry.custom_fields,
                entry_type: item.entry_type,
                created_at: now,
                updated_at: now,
                password_history: Vec::new(),
                breach_status: BreachStatus::Unknown,
                strength_score: if new_entry.password.is_empty() {
                    None
                } else {
                    Some(crate::breach::strength::analyze_password(&new_entry.password))
                },
                password_changed_at: now,
                encrypted_passkey: None,
                passkey_public_key: None,
                attachments: Vec::new(),
            };

            self.data.entries.push(entry);
            count += 1;
        }

        self.rebuild_search_index();
        self.save()?;
        Ok(count)
    }

    /// Exports decrypted vault entries to a CSV file.
    pub fn export_csv(&self, dest_path: &Path) -> crate::Result<()> {
        reject_vault_destination(&self.path, dest_path)?;
        write_sensitive_file_safely(dest_path, &self.export_csv_text()?)
    }

    pub fn export_csv_text(&self) -> crate::Result<zeroize::Zeroizing<String>> {
        let mut csv = zeroize::Zeroizing::new(String::from("Title,Username,Email,Password,URL,Notes,TOTP,Tags\n"));
        for entry in self.list_entries()? {
            {
                let dec = self.get_entry(entry.id)?;
                let esc = |s: &str| {
                    let trimmed = s.trim_start();
                    let safe = if s.starts_with('\t')
                        || s.starts_with('\r')
                        || trimmed.starts_with('=')
                        || trimmed.starts_with('+')
                        || trimmed.starts_with('-')
                        || trimmed.starts_with('@')
                        || trimmed.starts_with('\t')
                        || trimmed.starts_with('\r')
                    {
                        format!("'{}", s)
                    } else {
                        s.to_string()
                    };
                    format!("\"{}\"", safe.replace('"', "\"\"").replace('\n', " ").replace('\r', ""))
                };
                let totp = dec.totp_secret.as_deref().unwrap_or("");
                let tags = dec.tags.join(";");

                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    esc(&dec.title),
                    esc(&dec.username),
                    esc(&dec.email),
                    esc(&dec.password),
                    esc(&dec.url),
                    esc(&dec.notes),
                    esc(totp),
                    esc(&tags),
                ));
            }
        }
        Ok(csv)
    }

    /// Exports decrypted vault entries to a JSON file.
    pub fn export_json(&self, dest_path: &Path) -> crate::Result<()> {
        reject_vault_destination(&self.path, dest_path)?;
        write_sensitive_file_safely(dest_path, &self.export_json_text()?)
    }

    pub fn export_json_text(&self) -> crate::Result<zeroize::Zeroizing<String>> {
        let mut items = Vec::new();
        for entry in self.list_entries()? {
            {
                let dec = self.get_entry(entry.id)?;
                items.push(dec);
            }
        }
        let json = serde_json::to_string_pretty(&items)
            .map_err(|e| VaultError::SerializationError(format!("Failed to format JSON: {}", e)))?;
        Ok(zeroize::Zeroizing::new(json))
    }
}

fn reject_vault_destination(vault: &Path, destination: &Path) -> crate::Result<()> {
    if vault == destination || (destination.exists() && same_file::is_same_file(vault, destination)?) {
        return Err(VaultError::ExportError("Choose a different file: exporting here would overwrite the active vault".into()));
    }
    Ok(())
}

fn write_sensitive_file_safely(path: &Path, content: &str) -> crate::Result<()> {
    super::storage::atomic_write(path, content.as_bytes())
}
