//! Tag operations for VaultManager.
//!
//! Handles tag listing, creation, deletion, renaming, and default presets.

use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::Tag;

impl VaultManager {
    /// Return slice of all tags registered in the vault.
    pub fn tags(&self) -> &[Tag] {
        &self.data.tags
    }

    /// Add a new tag to the vault.
    pub fn add_tag(&mut self, name: &str, color: &str, icon: &str) -> crate::Result<Uuid> {
        let id = Uuid::new_v4();
        self.data.tags.push(Tag {
            id,
            name: name.to_string(),
            color: color.to_string(),
            icon: icon.to_string(),
        });
        self.save()?;
        Ok(id)
    }

    /// Delete a tag and remove it from all entries referencing it.
    pub fn delete_tag(&mut self, id: Uuid) -> crate::Result<()> {
        let tag_name = self
            .data
            .tags
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.name.clone());

        match tag_name {
            Some(name) => {
                self.data.tags.retain(|t| t.id != id);
                let mut entries_changed = false;
                for entry in &mut self.data.entries {
                    let initial_len = entry.tags.len();
                    entry.tags.retain(|t| t != &name);
                    if entry.tags.len() != initial_len {
                        entries_changed = true;
                    }
                }
                if entries_changed {
                    self.rebuild_search_index();
                }
                self.save()?;
                Ok(())
            }
            None => Err(VaultError::EntryNotFound(format!("Tag {} not found", id))),
        }
    }

    /// Update tag name, color, or icon, and update all entries referencing the old name.
    pub fn update_tag(&mut self, id: Uuid, name: &str, color: &str, icon: &str) -> crate::Result<()> {
        if let Some(tag) = self.data.tags.iter_mut().find(|t| t.id == id) {
            let old_name = tag.name.clone();
            tag.name = name.to_string();
            tag.color = color.to_string();
            tag.icon = icon.to_string();

            // Update all entries referencing this tag if the name changed
            if old_name != tag.name {
                let mut entries_changed = false;
                for entry in &mut self.data.entries {
                    for t in &mut entry.tags {
                        if *t == old_name {
                            *t = tag.name.clone();
                            entries_changed = true;
                        }
                    }
                }
                if entries_changed {
                    self.rebuild_search_index();
                }
            }
            self.save()?;
        }
        Ok(())
    }

    /// Default starter tags created on new vaults.
    #[allow(dead_code)]
    pub(crate) fn default_tags() -> Vec<Tag> {
        vec![
            Tag {
                id: Uuid::new_v4(),
                name: "Work".into(),
                color: "#5b8def".into(),
                icon: "briefcase".into(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Personal".into(),
                color: "#5acf7e".into(),
                icon: "user".into(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Finance".into(),
                color: "#f5a623".into(),
                icon: "credit-card".into(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Social".into(),
                color: "#bd7ee8".into(),
                icon: "users".into(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Development".into(),
                color: "#ef6b6b".into(),
                icon: "code".into(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::entry::NewEntry;
    use std::fs;
    use std::path::PathBuf;

    struct TestVault {
        path: PathBuf,
    }

    impl TestVault {
        fn new() -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("yntra_vault_test_tags_{}.vdb", Uuid::new_v4()));
            TestVault { path }
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn test_tag_search_index_invalidation_on_delete_and_update() {
        let test_vault = TestVault::new();
        let mut manager = VaultManager::create("tag-test-vault", "password123", &test_vault.path).unwrap();

        let tag_id = manager.add_tag("Production", "#ff0000", "server").unwrap();

        let entry = NewEntry {
            title: "Internal Server".to_string(),
            username: "admin".to_string(),
            password: "secret_password".to_string(),
            url: "https://10.0.0.1".to_string(),
            email: "admin@internal.net".to_string(),
            notes: "Internal infra".to_string(),
            tags: vec!["Production".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        };
        let entry_id = manager.add_entry(entry).unwrap();

        // Tag search matches entry
        let results = manager.search_entries("Production").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, entry_id);

        // Update tag name to "Staging"
        manager.update_tag(tag_id, "Staging", "#ffaa00", "server").unwrap();

        // Old tag name should no longer match
        let old_results = manager.search_entries("Production").unwrap();
        assert!(old_results.is_empty());

        // New tag name should match
        let new_results = manager.search_entries("Staging").unwrap();
        assert_eq!(new_results.len(), 1);
        assert_eq!(new_results[0].id, entry_id);

        // Delete tag
        manager.delete_tag(tag_id).unwrap();

        // Tag search should no longer match
        let after_delete = manager.search_entries("Staging").unwrap();
        assert!(after_delete.is_empty());

        // Title search still matches
        let title_results = manager.search_entries("Internal").unwrap();
        assert_eq!(title_results.len(), 1);
        assert_eq!(title_results[0].id, entry_id);
    }
}
