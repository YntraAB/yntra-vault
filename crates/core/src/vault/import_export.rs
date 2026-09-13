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

impl VaultManager {
    /// Checks an array of ParsedImportEntry against current vault entries for duplicates.
    pub fn check_import_duplicates(&self, entries: &mut [ParsedImportEntry]) -> usize {
        let mut dup_count = 0;
        for item in entries.iter_mut() {
            let t_lower = item.title.trim().to_lowercase();
            let u_lower = item.username.trim().to_lowercase();

            let match_found = self.data.entries.iter().any(|e| {
                let e_t = e.title.trim().to_lowercase();
                let e_u = e.username.trim().to_lowercase();
                let title_matches = !t_lower.is_empty() && e_t == t_lower;
                let user_matches = e_u == u_lower || u_lower.is_empty() || e_u.is_empty();
                title_matches && user_matches
            });

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
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;
        let entry_key = keys.entry_key.clone();
        let mut count = 0;

        let tag_colors = ["#3b82f6", "#10b981", "#8b5cf6", "#f59e0b", "#ec4899", "#06b6d4"];
        let mut color_idx = 0;

        for item in entries {
            let t_lower = item.title.trim().to_lowercase();
            let u_lower = item.username.trim().to_lowercase();

            let existing_id = self
                .data
                .entries
                .iter()
                .find(|e| {
                    let e_t = e.title.trim().to_lowercase();
                    let e_u = e.username.trim().to_lowercase();
                    let title_matches = !t_lower.is_empty() && e_t == t_lower;
                    let user_matches = e_u == u_lower || u_lower.is_empty() || e_u.is_empty();
                    title_matches && user_matches
                })
                .map(|e| e.id);

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
                        self.update_entry(target_id, update)?;
                        count += 1;
                        continue;
                    }
                    DuplicateStrategy::KeepBoth => {
                        // Fallthrough to add as new entry
                    }
                }
            }

            // Ensure all tags attached to item exist in vault's global tag list
            for tag_name in &item.tags {
                let tag_trimmed = tag_name.trim();
                if !tag_trimmed.is_empty()
                    && !self.data.tags.iter().any(|t| t.name.eq_ignore_ascii_case(tag_trimmed))
                {
                    self.data.tags.push(Tag {
                        id: Uuid::new_v4(),
                        name: tag_trimmed.to_string(),
                        color: tag_colors[color_idx % tag_colors.len()].to_string(),
                        icon: "tag".to_string(),
                    });
                    color_idx += 1;
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
        let mut csv = String::from("Title,Username,Email,Password,URL,Notes,TOTP,Tags\n");
        for entry in self.list_entries()? {
            if let Ok(dec) = self.get_entry(entry.id) {
                let esc = |s: &str| format!("\"{}\"", s.replace('"', "\"\"").replace('\n', " ").replace('\r', ""));
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
        std::fs::write(dest_path, csv)
            .map_err(|e| VaultError::InvalidFormat(format!("Failed to write CSV: {}", e)))
    }

    /// Exports decrypted vault entries to a JSON file.
    pub fn export_json(&self, dest_path: &Path) -> crate::Result<()> {
        let mut items = Vec::new();
        for entry in self.list_entries()? {
            if let Ok(dec) = self.get_entry(entry.id) {
                items.push(dec);
            }
        }
        let json = serde_json::to_string_pretty(&items)
            .map_err(|e| VaultError::SerializationError(format!("Failed to format JSON: {}", e)))?;
        std::fs::write(dest_path, json)
            .map_err(|e| VaultError::InvalidFormat(format!("Failed to write JSON: {}", e)))
    }
}
