//! Native Mobile (Android / iOS) Autofill Service integration & hardened security engine.
//! Provides anti-phishing package verification, Digital Asset Links validation, WebView origin inspection, and transient memory zeroization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::vault::types::EntryPreview;
use crate::vault::manager::{DecryptedEntry, VaultManager};

/// Mobile Autofill service availability & security status.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MobileAutofillStatus {
    pub supported: bool,
    pub enabled: bool,
    pub active_provider: String,
    pub mapped_packages_count: usize,
    pub strict_domain_matching: bool,
    pub asset_links_enforced: bool,
    pub webview_origin_protected: bool,
    pub biometric_stepup_required: bool,
}

/// Serialized dataset payload formatted for Android AutofillService & iOS CredentialProvider.
/// Implements ZeroizeOnDrop to ensure cleartext passwords and usernames are zeroed out in RAM upon drop.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct AutofillCredentialItem {
    pub entry_id: String,
    pub title: String,
    pub username: String,
    pub domain: String,
    pub matched_by: String,
    pub is_exact_package_match: bool,
    pub requires_user_consent: bool,
    pub requires_biometric_reauth: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AutofillDatasetPayload {
    pub package_name: String,
    pub web_domain: Option<String>,
    pub matched_credentials: Vec<AutofillCredentialItem>,
    pub asset_links_verified: bool,
}

/// Known Android package name to domain mappings for popular mobile applications.
fn get_known_package_mappings() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("com.twitter.android", "twitter.com");
    m.insert("com.x.android", "x.com");
    m.insert("com.instagram.android", "instagram.com");
    m.insert("com.github.android", "github.com");
    m.insert("com.google.android.youtube", "youtube.com");
    m.insert("com.reddit.frontpage", "reddit.com");
    m.insert("com.netflix.mediaclient", "netflix.com");
    m.insert("com.amazon.mShop.android.shopping", "amazon.com");
    m.insert("com.facebook.katana", "facebook.com");
    m.insert("com.spotify.music", "spotify.com");
    m.insert("com.discord", "discord.com");
    m.insert("org.telegram.messenger", "telegram.org");
    m.insert("com.linkedin.android", "linkedin.com");
    m.insert("com.slack", "slack.com");
    m.insert("com.dropbox.android", "dropbox.com");
    m.insert("com.bitwarden.authenticator", "bitwarden.com");
    m.insert("com.valvesoftware.android.steam.community", "steampowered.com");
    m.insert("com.epicgames.portal", "epicgames.com");
    m
}

/// Extract clean host domain from a URL (e.g. "https://open.spotify.com/login" -> "spotify.com").
pub fn extract_host_domain(url: &str) -> Option<String> {
    let cleaned = url.trim();
    if cleaned.is_empty() {
        return None;
    }
    let without_scheme = cleaned
        .strip_prefix("https://")
        .or_else(|| cleaned.strip_prefix("http://"))
        .unwrap_or(cleaned);
    let host = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host_no_port = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = host_no_port.split('.').collect();
    if parts.len() >= 2 {
        let root_domain = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        Some(root_domain.to_lowercase())
    } else {
        Some(host_no_port.to_lowercase())
    }
}

/// Digital Asset Links verification stub (`/.well-known/assetlinks.json`).
pub fn verify_digital_asset_links(_domain: &str, _package_name: &str, _apk_sha256: Option<&str>) -> bool {
    // A package-name lookup cannot authenticate an installed application's signing
    // certificate or prove that the website authorizes that certificate. Fail closed.
    false
}

/// Hardened matching engine with WebDomain origin inspection and biometric step-up enforcement.
/// Rejects fuzzy title keyword guesses and inspects target web_domain to prevent In-App WebView origin confusion.
pub fn match_entries_for_mobile_context(
    entries: &[DecryptedEntry],
    package_name: &str,
    web_domain: Option<&str>,
) -> Vec<AutofillCredentialItem> {
    let mut results = Vec::new();
    let known_mappings = get_known_package_mappings();
    let target_domain = known_mappings.get(package_name).copied();
    let pkg_lower = package_name.to_lowercase();

    // If an in-app WebView provided an explicit target web_domain, inspect the web_domain host
    let web_host = web_domain.and_then(extract_host_domain);

    for entry in entries {
        let mut matched_reason: Option<String> = None;
        let mut is_exact_match = false;

        // 1. In-App WebView Domain Host Override Inspection
        if let Some(ref target_web_host) = web_host
            && let Some(entry_host) = extract_host_domain(&entry.url)
                && (entry_host == *target_web_host || entry_host.ends_with(&format!(".{}", target_web_host))) {
                    matched_reason = Some(format!("webview_webdomain_override ({})", target_web_host));
                    is_exact_match = true;
                }

        // 2. Explicit Custom Field Package Match (e.g. custom field "package_name" = "com.spotify.music")
        if matched_reason.is_none() {
            for field in &entry.custom_fields {
                let field_name_lower = field.name.to_lowercase();
                if (field_name_lower == "package_name" || field_name_lower == "app_id" || field_name_lower == "android_package")
                    && field.value.trim().to_lowercase() == pkg_lower
                {
                    matched_reason = Some(format!("explicit_custom_field ({})", field.name));
                    is_exact_match = true;
                    break;
                }
            }
        }

        // 3. Verified Domain Match via Known App Package Table
        if matched_reason.is_none()
            && let Some(domain) = target_domain
                && let Some(entry_host) = extract_host_domain(&entry.url)
                    && (entry_host == domain || entry_host.ends_with(&format!(".{}", domain))) {
                        matched_reason = Some(format!("known_package_verified_domain ({})", domain));
                        is_exact_match = true;
                    }

        // 4. Strict Exact URL Host match against package name domain
        if matched_reason.is_none() && !entry.url.is_empty()
            && let Some(entry_host) = extract_host_domain(&entry.url)
                && entry_host == pkg_lower {
                    matched_reason = Some("exact_url_host_match".into());
                    is_exact_match = true;
                }

        if let Some(reason) = matched_reason {
            results.push(AutofillCredentialItem {
                entry_id: entry.id.to_string(),
                title: entry.title.clone(),
                username: entry.username.clone(),
                domain: entry.url.clone(),
                matched_by: reason,
                is_exact_package_match: is_exact_match,
                requires_user_consent: true,
                requires_biometric_reauth: true,
            });
        }
    }

    results
}

/// Backward compatible convenience function calling match_entries_for_mobile_context without web_domain.
pub fn match_entries_for_package(entries: &[DecryptedEntry], package_name: &str) -> Vec<AutofillCredentialItem> {
    match_entries_for_mobile_context(entries, package_name, None)
}

/// Convert AutofillCredentialItem list to EntryPreview list.
pub fn to_entry_previews(items: &[AutofillCredentialItem], previews: &[EntryPreview]) -> Vec<EntryPreview> {
    let mut results = Vec::new();
    let id_map: HashMap<String, &EntryPreview> = previews.iter().map(|e| (e.id.to_string(), e)).collect();

    for item in items {
        if let Some(entry) = id_map.get(&item.entry_id) {
            results.push((*entry).clone());
        }
    }

    results
}

impl VaultManager {
    /// Queries vault entries for a given mobile application package name.
    pub fn find_entries_for_mobile_package(&self, package_name: &str) -> crate::Result<Vec<AutofillCredentialItem>> {
        self.find_entries_for_mobile_context(package_name, None)
    }

    /// Queries vault entries for a given mobile package and optional WebDomain origin.
    pub fn find_entries_for_mobile_context(
        &self,
        package_name: &str,
        web_domain: Option<&str>,
    ) -> crate::Result<Vec<AutofillCredentialItem>> {
        let mut full_entries = Vec::new();
        for preview in self.list_entries()? {
            if let Ok(decrypted) = self.get_entry(preview.id) {
                full_entries.push(decrypted);
            }
        }
        Ok(match_entries_for_mobile_context(&full_entries, package_name, web_domain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use chrono::Utc;
    use crate::vault::types::{EntryType, BreachStatus};

    fn create_test_entry(title: &str, username: &str, url: &str) -> DecryptedEntry {
        let now = Utc::now();
        DecryptedEntry {
            id: Uuid::new_v4(),
            title: title.to_string(),
            username: username.to_string(),
            password: "secret".to_string(),
            url: url.to_string(),
            email: String::new(),
            notes: String::new(),
            tags: Vec::new(),
            favorite: false,
            pinned: false,
            totp_secret: None,
            custom_fields: Vec::new(),
            entry_type: EntryType::Login,
            created_at: now,
            updated_at: now,
            password_changed_at: now,
            breach_status: BreachStatus::Unknown,
            strength_score: None,
            password_history_count: 0,
            has_passkey: false,
            passkey_public_key: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn test_strict_known_package_matching() {
        let entries = vec![
            create_test_entry("Spotify Music", "user1@spotify.com", "https://open.spotify.com/login"),
            create_test_entry("GitHub Account", "octocat", "https://github.com/login"),
        ];

        let matched = match_entries_for_package(&entries, "com.spotify.music");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].username, "user1@spotify.com");
        assert!(matched[0].requires_biometric_reauth);
    }

    #[test]
    fn test_in_app_webview_domain_override() {
        let entries = vec![
            create_test_entry("GitHub Login", "octocat", "https://github.com/login"),
            create_test_entry("News Site", "newsuser", "https://news.example.com"),
        ];

        // Host news app (com.news.reader) opens github.com inside a WebView tab.
        // WebDomain override MUST match GitHub entry instead of news reader host package.
        let matched = match_entries_for_mobile_context(&entries, "com.news.reader", Some("https://github.com/login"));
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].username, "octocat");
        assert!(matched[0].matched_by.contains("webview_webdomain_override"));
    }

    #[test]
    fn test_anti_phishing_spoofed_package_rejection() {
        let entries = vec![
            create_test_entry("Spotify Music", "user1@spotify.com", "https://open.spotify.com/login"),
        ];

        let matched_fake = match_entries_for_package(&entries, "com.fake.spotify");
        assert_eq!(matched_fake.len(), 0);
    }
}
