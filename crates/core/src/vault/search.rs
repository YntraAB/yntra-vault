use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
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

/// Generate search tokens (unigrams, bigrams, and trigrams) for indexing text in the zero-disclosure index.
pub fn generate_index_tokens(text: &str) -> Vec<String> {
    let normalized = text.to_lowercase();
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut tokens = Vec::new();

    // 1. Unigrams (skip whitespace)
    for &ch in &chars {
        if !ch.is_whitespace() {
            tokens.push(ch.to_string());
        }
    }

    // 2. Bigrams (skip purely whitespace pairs)
    if chars.len() >= 2 {
        for i in 0..=chars.len() - 2 {
            if !chars[i].is_whitespace() || !chars[i + 1].is_whitespace() {
                tokens.push(chars[i..i + 2].iter().collect());
            }
        }
    }

    // 3. Trigrams (skip purely whitespace triplets)
    if chars.len() >= 3 {
        for i in 0..=chars.len() - 3 {
            if !chars[i].is_whitespace() || !chars[i + 1].is_whitespace() || !chars[i + 2].is_whitespace() {
                tokens.push(chars[i..i + 3].iter().collect());
            }
        }
    }

    tokens.sort_unstable();
    tokens.dedup();
    tokens
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
    pub fn rebuild_search_index(&mut self) {
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

                let tokens = generate_index_tokens(&text_to_hash);
                for token in tokens {
                    let hashed = hash_trigram(&token, &keys.search_key.bytes);
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

            let tokens = generate_index_tokens(&text_to_hash);
            for token in tokens {
                let hashed = hash_trigram(&token, &keys.search_key.bytes);
                self.search_index.entry(hashed).or_default().push(id);
            }
        }
    }

    /// Remove an entry's ID from the HMAC-SHA256 trigram index, automatically pruning empty buckets.
    pub(crate) fn remove_entry_from_index(&mut self, id: Uuid) {
        self.search_index.retain(|_, list| {
            list.retain(|x| *x != id);
            !list.is_empty()
        });
    }

    /// Zero-Disclosure Search: search entries by query string using HMAC trigrams.
    /// Returns matching entry previews without decrypting passwords or exposing search strings.
    pub fn search_entries(&self, query: &str) -> crate::Result<Vec<EntryPreview>> {
        if !self.is_unlocked() {
            return Err(crate::error::VaultError::VaultLocked);
        }

        let query = query.trim();
        if query.is_empty() {
            return self.list_entries();
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

        // Pre-filter matched entry IDs before allocating any preview structs
        let matched_ids: HashSet<Uuid> = match_counts
            .iter()
            .filter(|(_, count)| **count >= threshold)
            .map(|(id, _)| *id)
            .collect();

        if matched_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Allocate and construct EntryPreview only for the matched subset
        let mut results: Vec<EntryPreview> = self
            .data
            .entries
            .iter()
            .filter(|e| matched_ids.contains(&e.id))
            .map(|e| {
                let age = (Utc::now() - e.password_changed_at).num_days();
                EntryPreview {
                    id: e.id,
                    title: e.title.clone(),
                    username: e.username.clone(),
                    url: e.url.clone(),
                    email: e.email.clone(),
                    tags: e.tags.clone(),
                    favorite: e.favorite,
                    pinned: e.pinned,
                    has_totp: e.encrypted_totp_secret.is_some(),
                    entry_type: e.entry_type.clone(),
                    updated_at: e.updated_at,
                    breach_status: e.breach_status.clone(),
                    strength_score: e.strength_score.clone(),
                    password_age_days: age,
                    has_passkey: e.encrypted_passkey.is_some(),
                    attachment_count: e.attachments.len(),
                }
            })
            .collect();

        // Sort results by relevance (highest match count first, then prefix/exact title match)
        let query_lower = query.to_lowercase();
        results.sort_by(|a, b| {
            let count_a = match_counts.get(&a.id).unwrap_or(&0);
            let count_b = match_counts.get(&b.id).unwrap_or(&0);
            if count_b != count_a {
                return count_b.cmp(count_a);
            }
            let a_title_lower = a.title.to_lowercase();
            let b_title_lower = b.title.to_lowercase();
            let a_exact = a_title_lower == query_lower;
            let b_exact = b_title_lower == query_lower;
            if a_exact != b_exact {
                return b_exact.cmp(&a_exact);
            }
            let a_starts = a_title_lower.starts_with(&query_lower);
            let b_starts = b_title_lower.starts_with(&query_lower);
            if a_starts != b_starts {
                return b_starts.cmp(&a_starts);
            }
            a.title.cmp(&b.title)
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
    fn test_generate_index_tokens() {
        let tokens = generate_index_tokens("go");
        assert!(tokens.contains(&"g".to_string()));
        assert!(tokens.contains(&"o".to_string()));
        assert!(tokens.contains(&"go".to_string()));

        let tokens_x = generate_index_tokens("x");
        assert_eq!(tokens_x, vec!["x"]);

        let tokens_google = generate_index_tokens("google");
        // Contains unigrams
        assert!(tokens_google.contains(&"g".to_string()));
        assert!(tokens_google.contains(&"o".to_string()));
        // Contains bigrams
        assert!(tokens_google.contains(&"go".to_string()));
        assert!(tokens_google.contains(&"oo".to_string()));
        // Contains trigrams
        assert!(tokens_google.contains(&"goo".to_string()));
        assert!(tokens_google.contains(&"gle".to_string()));
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
            attachments: None,
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
            attachments: None,
        };

        let entry3 = NewEntry {
            title: "X".to_string(),
            username: "elon".to_string(),
            password: "xpassword".to_string(),
            url: "https://x.com".to_string(),
            email: "x@x.com".to_string(),
            notes: "Microblogging platform".to_string(),
            tags: vec!["Social".to_string(), "AI".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        };

        let entry4 = NewEntry {
            title: "Facebook".to_string(),
            username: "fb_user".to_string(),
            password: "fbpassword".to_string(),
            url: "https://facebook.com".to_string(),
            email: "user@facebook.com".to_string(),
            notes: "Social network".to_string(),
            tags: vec!["fb".to_string(), "Social".to_string()],
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: None,
            generate_passkey: None,
            attachments: None,
        };

        let id1 = manager.add_entry(entry1).unwrap();
        let id2 = manager.add_entry(entry2).unwrap();
        let id3 = manager.add_entry(entry3).unwrap();
        let id4 = manager.add_entry(entry4).unwrap();

        // Test searching for Google
        let results = manager.search_entries("Google").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, id1);

        // Test searching for GitHub
        let results = manager.search_entries("git").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, id2);

        // Sub-3-letter searches (2-letter queries)
        let results_go = manager.search_entries("go").unwrap();
        assert!(!results_go.is_empty());
        assert_eq!(results_go[0].id, id1);

        let results_gi = manager.search_entries("gi").unwrap();
        assert!(!results_gi.is_empty());
        assert_eq!(results_gi[0].id, id2);

        let results_ai = manager.search_entries("ai").unwrap();
        assert!(!results_ai.is_empty());
        assert!(results_ai.iter().any(|r| r.id == id3));

        let results_fb = manager.search_entries("fb").unwrap();
        assert!(!results_fb.is_empty());
        assert_eq!(results_fb[0].id, id4);

        // Sub-3-letter searches (1-letter query)
        let results_x = manager.search_entries("x").unwrap();
        assert!(!results_x.is_empty());
        assert_eq!(results_x[0].id, id3);

        // Whitespace trimmed search
        let results_trimmed = manager.search_entries("  go  ").unwrap();
        assert!(!results_trimmed.is_empty());
        assert_eq!(results_trimmed[0].id, id1);

        // Test searching for nonexistent
        let results = manager.search_entries("nonexistent").unwrap();
        assert!(results.is_empty());

        // Test index removal & bucket pruning
        manager.remove_entry_from_index(id1);
        let results_after = manager.search_entries("Google").unwrap();
        assert!(results_after.is_empty());
        assert!(!manager.search_index.values().any(|v| v.is_empty()));
    }
}
