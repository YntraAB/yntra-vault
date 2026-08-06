//! Zero-Disclosure search index using HMAC-SHA256 trigrams.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;
use crate::vault::manager::VaultManager;
use crate::vault::types::EntryPreview;

type HmacSha256 = Hmac<Sha256>;

/// Generate trigrams of a text string for indexing.
/// If the text is shorter than 3 characters, returns the lowercased text itself as a single token.
pub fn generate_trigrams(text: &str) -> Vec<String> {
    let normalized = text.to_lowercase();
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() < 3 {
        return vec![normalized];
    }

    let mut trigrams = Vec::new();
    for i in 0..=chars.len() - 3 {
        let trigram: String = chars[i..i + 3].iter().collect();
        trigrams.push(trigram);
    }
    trigrams
}

/// Computes the HMAC-SHA256 of a trigram using the search key and truncates it to 8 bytes.
pub fn hash_trigram(trigram: &str, key: &[u8; 32]) -> [u8; 8] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts 32-byte key");
    mac.update(trigram.as_bytes());
    let result = mac.finalize().into_bytes();
    let mut truncated = [0u8; 8];
    truncated.copy_from_slice(&result[..8]);
    truncated
}

impl VaultManager {
    /// Rebuild the in-memory HMAC-SHA256 trigram search index from all current entries.
    pub(crate) fn rebuild_search_index(&mut self) {
        let mut index = std::collections::HashMap::new();
        if let Some(ref keys) = self.keys {
            for entry in &self.data.entries {
                let mut text_to_hash = format!(
                    "{} {} {} {}",
                    entry.title, entry.username, entry.url, entry.email
                );
                for tag in &entry.tags {
                    text_to_hash.push(' ');
                    text_to_hash.push_str(tag);
                }

                let trigrams = generate_trigrams(&text_to_hash);
                for trigram in trigrams {
                    let hashed = hash_trigram(&trigram, &keys.search_key.bytes);
                    index.entry(hashed).or_insert_with(Vec::new).push(entry.id);
                }
            }
        }
        self.search_index = index;
    }

    /// Add an entry's searchable fields to the HMAC-SHA256 trigram index.
    pub(crate) fn add_entry_to_index(
        &mut self,
        id: Uuid,
        title: &str,
        username: &str,
        url: &str,
        email: &str,
        tags: &[String],
    ) {
        if let Some(ref keys) = self.keys {
            let mut text_to_hash = format!(
                "{} {} {} {}",
                title, username, url, email
            );
            for tag in tags {
                text_to_hash.push(' ');
                text_to_hash.push_str(tag);
            }

            let trigrams = generate_trigrams(&text_to_hash);
            for trigram in trigrams {
                let hashed = hash_trigram(&trigram, &keys.search_key.bytes);
                self.search_index.entry(hashed).or_insert_with(Vec::new).push(id);
            }
        }
    }

    /// Remove an entry's ID from the HMAC-SHA256 trigram index.
    pub(crate) fn remove_entry_from_index(&mut self, id: Uuid) {
        for list in self.search_index.values_mut() {
            list.retain(|x| *x != id);
        }
    }

    /// Zero-Disclosure Search: search entries by query string using HMAC trigrams.
    /// Returns matching entry previews without decrypting passwords or exposing search strings.
    pub fn search_entries(&self, query: &str) -> crate::Result<Vec<EntryPreview>> {
        if !self.is_unlocked() {
            return Err(crate::error::VaultError::VaultLocked);
        }

        let all = self.list_entries()?;
        if query.is_empty() {
            return Ok(all);
        }

        let keys = self.keys.as_ref().ok_or(crate::error::VaultError::VaultLocked)?;
        let query_trigrams = generate_trigrams(query);
        let mut match_counts = std::collections::HashMap::new();

        for trigram in &query_trigrams {
            let hashed = hash_trigram(trigram, &keys.search_key.bytes);
            if let Some(entry_ids) = self.search_index.get(&hashed) {
                for id in entry_ids {
                    *match_counts.entry(*id).or_insert(0) += 1;
                }
            }
        }

        let threshold = if query_trigrams.len() <= 2 {
            1
        } else {
            // Match at least 80% of query trigrams (allowing for slight fuzziness)
            ((query_trigrams.len() as f64) * 0.8).floor() as usize
        };

        let mut results: Vec<EntryPreview> = all
            .into_iter()
            .filter(|e| {
                if let Some(&count) = match_counts.get(&e.id) {
                    count >= threshold
                } else {
                    false
                }
            })
            .collect();

        // Sort results by relevance (highest match count first)
        results.sort_by(|a, b| {
            let count_a = match_counts.get(&a.id).unwrap_or(&0);
            let count_b = match_counts.get(&b.id).unwrap_or(&0);
            count_b.cmp(count_a)
        });

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trigrams_normal() {
        let trigrams = generate_trigrams("google");
        assert_eq!(trigrams, vec!["goo", "oog", "ogl", "gle"]);
    }

    #[test]
    fn test_generate_trigrams_short() {
        let trigrams = generate_trigrams("go");
        assert_eq!(trigrams, vec!["go"]);
    }

    #[test]
    fn test_hash_trigram_deterministic() {
        let key = [42u8; 32];
        let hash1 = hash_trigram("goo", &key);
        let hash2 = hash_trigram("goo", &key);
        assert_eq!(hash1, hash2);

        let hash3 = hash_trigram("oog", &key);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_search_zero_disclosure() {
        use std::path::PathBuf;
        use std::fs;
        use crate::vault::manager::NewEntry;

        struct TestVault {
            path: PathBuf,
        }
        impl TestVault {
            fn new() -> Self {
                let mut path = std::env::temp_dir();
                path.push(format!("yntra_vault_test_search_{}.vdb", Uuid::new_v4()));
                TestVault { path }
            }
        }
        impl Drop for TestVault {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.path);
            }
        }

        let test_vault = TestVault::new();
        let mut manager = VaultManager::create("search-test-vault", "password", &test_vault.path).unwrap();

        let entry1 = NewEntry {
            title: "Google Workspace".to_string(),
            username: "user1".to_string(),
            password: "password123".to_string(),
            url: "https://google.com".to_string(),
            email: "user1@gmail.com".to_string(),
            notes: "Work email".to_string(),
            tags: vec!["Google".to_string(), "Work".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
        };

        let entry2 = NewEntry {
            title: "GitHub Dev Account".to_string(),
            username: "gituser".to_string(),
            password: "gitpassword".to_string(),
            url: "https://github.com".to_string(),
            email: "git@github.com".to_string(),
            notes: "Dev coding repo".to_string(),
            tags: vec!["GitHub".to_string(), "Coding".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
        };

        let id1 = manager.add_entry(entry1).unwrap();
        let id2 = manager.add_entry(entry2).unwrap();

        // Test searching for Google
        let results = manager.search_entries("Google").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, id1);

        // Test searching for GitHub
        let results = manager.search_entries("git").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, id2);

        // Test searching for nonexistent
        let results = manager.search_entries("nonexistent").unwrap();
        assert!(results.is_empty());
    }
}
