//! Tag operations for VaultManager.
//!
//! Handles tag listing, creation, deletion, renaming, and default presets.

use uuid::Uuid;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::Tag;

fn normalize_tag_url(value: &str) -> crate::Result<String> {
    let value = value.trim();
    if value.is_empty() { return Ok(String::new()); }
    if value.len() > 2048 || value.chars().any(|c| c.is_control()) || value.contains('\\') {
        return Err(VaultError::InvalidState("Invalid default URL".into()));
    }
    let candidate = if value.contains("://") { value.to_string() } else { format!("https://{value}") };
    let url = url::Url::parse(&candidate).map_err(|_| VaultError::InvalidState("Invalid default URL".into()))?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some() {
        return Err(VaultError::InvalidState("Use an HTTP or HTTPS URL without embedded credentials".into()));
    }
    Ok(url.to_string())
}

impl VaultManager {
    /// Return slice of all tags registered in the vault.
    pub fn tags(&self) -> &[Tag] {
        &self.data.tags
    }

    /// Add a new tag to the vault.
    pub fn add_tag(&mut self, name: &str, color: &str, icon: &str) -> crate::Result<Uuid> {
        self.add_tag_with_url(name, color, icon, "")
    }

    pub fn add_tag_with_url(&mut self, name: &str, color: &str, icon: &str, default_url: &str) -> crate::Result<Uuid> {
        let url = normalize_tag_url(default_url)?;
        self.transactional_data_change(|manager| manager.add_tag_inner(name, color, icon, &url))
    }

    fn add_tag_inner(&mut self, name: &str, color: &str, icon: &str, default_url: &str) -> crate::Result<Uuid> {
        crate::vault::validation::validate_display_name(name)?;
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        if self.data.tags.iter().any(|tag| tag.name == name) {
            return Err(VaultError::InvalidState("A tag with this name already exists".into()));
        }
        let id = Uuid::new_v4();
        self.data.tags.push(Tag {
            id,
            name: name.to_string(),
            color: color.to_string(),
            icon: icon.to_string(),
            default_url: default_url.to_string(),
        });
        self.save()?;
        Ok(id)
    }

    /// Delete a tag and remove it from all entries referencing it.
    pub fn delete_tag(&mut self, id: Uuid) -> crate::Result<()> {
        self.transactional_data_change(|manager| manager.delete_tag_inner(id))
    }

    fn delete_tag_inner(&mut self, id: Uuid) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
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
                for entry in self.data.entries.iter_mut().chain(self.data.trash.iter_mut().map(|t| &mut t.entry)) {
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
        self.update_tag_with_url(id, name, color, icon, None)
    }

    pub fn update_tag_with_url(&mut self, id: Uuid, name: &str, color: &str, icon: &str, default_url: Option<&str>) -> crate::Result<()> {
        let url = default_url.map(normalize_tag_url).transpose()?;
        self.transactional_data_change(|manager| manager.update_tag_inner(id, name, color, icon, url.as_deref()))
    }

    fn update_tag_inner(&mut self, id: Uuid, name: &str, color: &str, icon: &str, default_url: Option<&str>) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        if self.data.tags.iter().any(|tag| tag.id != id && tag.name == name) {
            return Err(VaultError::InvalidState("A tag with this name already exists".into()));
        }
        if let Some(tag) = self.data.tags.iter_mut().find(|t| t.id == id) {
            if tag.name != name {
                crate::vault::validation::validate_display_name(name)?;
            }
            let old_name = tag.name.clone();
            tag.name = name.to_string();
            tag.color = color.to_string();
            tag.icon = icon.to_string();
            if let Some(url) = default_url { tag.default_url = url.to_string(); }

            // Update all entries referencing this tag if the name changed
            if old_name != tag.name {
                let mut entries_changed = false;
                for entry in self.data.entries.iter_mut().chain(self.data.trash.iter_mut().map(|t| &mut t.entry)) {
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
        } else {
            return Err(VaultError::EntryNotFound(format!("Tag {} not found", id)));
        }
        Ok(())
    }

    /// Reorder tags according to the provided list of IDs.
    pub fn reorder_tags(&mut self, tag_ids: &[Uuid]) -> crate::Result<()> {
        self.transactional_data_change(|manager| manager.reorder_tags_inner(tag_ids))
    }

    fn reorder_tags_inner(&mut self, tag_ids: &[Uuid]) -> crate::Result<()> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        let mut new_tags = Vec::with_capacity(self.data.tags.len());
        for id in tag_ids {
            if let Some(pos) = self.data.tags.iter().position(|t| t.id == *id) {
                new_tags.push(self.data.tags.remove(pos));
            }
        }
        // Append any remaining tags that were not in tag_ids
        new_tags.append(&mut self.data.tags);
        self.data.tags = new_tags;
        self.save()?;
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
                icon: "briefcase".into(), default_url: String::new(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Personal".into(),
                color: "#5acf7e".into(),
                icon: "user".into(), default_url: String::new(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Finance".into(),
                color: "#f5a623".into(),
                icon: "credit-card".into(), default_url: String::new(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Social".into(),
                color: "#bd7ee8".into(),
                icon: "users".into(), default_url: String::new(),
            },
            Tag {
                id: Uuid::new_v4(),
                name: "Development".into(),
                color: "#ef6b6b".into(),
                icon: "code".into(), default_url: String::new(),
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
    fn duplicate_tag_names_and_missing_updates_leave_disk_unchanged() {
        let fixture = TestVault::new();
        let mut manager = VaultManager::create("Tags", "Synthetic-Tags-2026!", &fixture.path).unwrap();
        let first = manager.add_tag("One", "red", "tag").unwrap();
        let second = manager.add_tag("Two", "blue", "tag").unwrap();
        let before = fs::read(&fixture.path).unwrap();
        assert!(manager.add_tag("One", "green", "tag").is_err());
        assert!(manager.update_tag(second, "One", "green", "tag").is_err());
        assert!(manager.update_tag(Uuid::new_v4(), "Missing", "green", "tag").is_err());
        assert_eq!(fs::read(&fixture.path).unwrap(), before);
        assert_eq!(manager.tags().iter().find(|tag| tag.id == first).unwrap().name, "One");
        assert_eq!(manager.tags().iter().find(|tag| tag.id == second).unwrap().name, "Two");
    }

    #[test]
    fn test_tag_search_index_invalidation_on_delete_and_update() {
        let test_vault = TestVault::new();
        let mut manager = VaultManager::create("tag-test-vault", "fixture-password123", &test_vault.path).unwrap();

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

    #[test]
    fn test_reorder_tags() {
        let test_vault = TestVault::new();
        let mut manager = VaultManager::create("reorder-tags-vault", "fixture-password123", &test_vault.path).unwrap();

        let id1 = manager.add_tag("Tag1", "#ff0000", "tag").unwrap();
        let id2 = manager.add_tag("Tag2", "#00ff00", "tag").unwrap();
        let id3 = manager.add_tag("Tag3", "#0000ff", "tag").unwrap();

        assert_eq!(manager.tags()[0].name, "Tag1");
        assert_eq!(manager.tags()[1].name, "Tag2");
        assert_eq!(manager.tags()[2].name, "Tag3");

        manager.reorder_tags(&[id3, id1, id2]).unwrap();

        assert_eq!(manager.tags()[0].id, id3);
        assert_eq!(manager.tags()[1].id, id1);
        assert_eq!(manager.tags()[2].id, id2);

        // Verify persistence to encrypted disk file
        let reloaded = VaultManager::open(&test_vault.path, "fixture-password123").unwrap();
        assert_eq!(reloaded.tags()[0].id, id3);
        assert_eq!(reloaded.tags()[1].id, id1);
        assert_eq!(reloaded.tags()[2].id, id2);
    }
}
