//! Security audit and health score calculation for VaultManager.
//!
//! Analyzes vault entries for breached, weak, reused, or outdated credentials,
//! missing 2FA on critical services, and transient memory zeroization.

use chrono::Utc;
use rand::RngCore;
use std::collections::HashMap;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::VaultError;
use crate::vault::manager::VaultManager;
use crate::vault::types::{
    BreachStatus, IssueSeverity, IssueType, SecurityAudit, SecurityIssue, StrengthLevel,
    StrengthScore,
};

/// Helper to check if a service domain is known to support 2FA.
fn is_important_service(url: &str) -> bool {
    let important = [
        "google", "gmail", "github", "amazon", "aws", "microsoft",
        "apple", "facebook", "twitter", "x.com", "dropbox", "slack",
        "discord", "paypal", "stripe", "cloudflare", "digitalocean",
        "linkedin", "instagram", "reddit", "twitch",
    ];
    let url_lower = url.to_lowercase();
    important.iter().any(|s| url_lower.contains(s))
}

impl VaultManager {
    /// Generate a full security audit report for the unlocked vault.
    pub fn security_audit(&self) -> crate::Result<SecurityAudit> {
        if !self.is_unlocked() {
            return Err(VaultError::VaultLocked);
        }
        let keys = self.keys.as_ref().ok_or(VaultError::VaultLocked)?;

        let mut issues = Vec::new();
        let mut breached = 0usize;
        let mut weak = 0usize;
        let mut old = 0usize;
        let mut no_2fa = 0usize;
        let mut reused = 0usize;

        let total = self.data.entries.len();

        // Ephemeral keyed hash map for zero-plaintext password reuse detection
        let mut ephemeral_key = [0u8; 32];
        rand::rng().fill_bytes(&mut ephemeral_key);
        let mut hash_map: HashMap<[u8; 32], Vec<(Uuid, String)>> = HashMap::new();

        struct EntryAuditMeta {
            blind_hash: Option<[u8; 32]>,
            strength: Option<StrengthScore>,
        }

        let mut audit_items: Vec<EntryAuditMeta> = Vec::with_capacity(total);

        for entry in &self.data.entries {
            let pwd_bytes =
                Self::decrypt_entry_field(&entry.encrypted_password, &keys.entry_key, &entry.id, "password")?;

            if !pwd_bytes.is_empty() {
                let blind_hash = *blake3::keyed_hash(&ephemeral_key, &pwd_bytes).as_bytes();
                hash_map
                    .entry(blind_hash)
                    .or_default()
                    .push((entry.id, entry.title.clone()));

                let strength = match &entry.strength_score {
                    Some(score) => score.clone(),
                    None => {
                        let pwd_str = zeroize::Zeroizing::new(
                            String::from_utf8(pwd_bytes.to_vec())
                                .map_err(|e| VaultError::DecryptionError(e.to_string()))?,
                        );
                        crate::breach::strength::analyze_password(&pwd_str)
                    }
                };

                audit_items.push(EntryAuditMeta {
                    blind_hash: Some(blind_hash),
                    strength: Some(strength),
                });
            } else {
                audit_items.push(EntryAuditMeta {
                    blind_hash: None,
                    strength: None,
                });
            }
        }

        for (i, entry) in self.data.entries.iter().enumerate() {
            let item = &audit_items[i];

            // Breach status
            if let BreachStatus::Breached { breach_count, .. } = &entry.breach_status {
                breached += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::Breached,
                    severity: IssueSeverity::Critical,
                    description: format!("Password found in {} data breaches", breach_count),
                });
            }

            // Perform password strength, reuse, and age audits only if password is non-empty
            if let Some(blind_hash) = &item.blind_hash {
                // Weak password (fallback to real-time calculation if None)
                if let Some(score) = &item.strength {
                    if score.level <= StrengthLevel::Weak {
                        weak += 1;
                        issues.push(SecurityIssue {
                            entry_id: entry.id,
                            entry_title: entry.title.clone(),
                            issue_type: IssueType::WeakPassword,
                            severity: IssueSeverity::Warning,
                            description: format!(
                                "Password strength: {:?} ({:.0} bits entropy)",
                                score.level, score.entropy_bits
                            ),
                        });
                    }
                }

                // Reused password
                if let Some(duplicates) = hash_map.get(blind_hash) {
                    if duplicates.len() > 1 {
                        reused += 1;
                        let other_services: Vec<String> = duplicates
                            .iter()
                            .filter(|(dup_id, _)| dup_id != &entry.id)
                            .map(|(_, dup_title)| dup_title.clone())
                            .collect();
                        issues.push(SecurityIssue {
                            entry_id: entry.id,
                            entry_title: entry.title.clone(),
                            issue_type: IssueType::ReusedPassword,
                            severity: IssueSeverity::Warning,
                            description: format!("Password is reused on: {}", other_services.join(", ")),
                        });
                    }
                }

                // Old password (> 90 days)
                let age_days = (Utc::now() - entry.password_changed_at).num_days();
                if age_days > 90 {
                    old += 1;
                    issues.push(SecurityIssue {
                        entry_id: entry.id,
                        entry_title: entry.title.clone(),
                        issue_type: IssueType::OldPassword,
                        severity: IssueSeverity::Info,
                        description: format!("Password hasn't been changed in {} days", age_days),
                    });
                }
            }

            // Missing 2FA on important accounts
            if entry.encrypted_totp_secret.is_none() && is_important_service(&entry.url) {
                no_2fa += 1;
                issues.push(SecurityIssue {
                    entry_id: entry.id,
                    entry_title: entry.title.clone(),
                    issue_type: IssueType::Missing2FA,
                    severity: IssueSeverity::Warning,
                    description: "This service supports 2FA but none is configured".to_string(),
                });
            }
        }

        // Zeroize ephemeral key
        ephemeral_key.zeroize();

        // Calculate health score (0-100), clamping penalty to prevent u8 overflow
        let raw_penalty = breached * 20 + weak * 10 + reused * 15 + old * 2 + no_2fa * 5;
        let issue_penalty = raw_penalty.min(100) as u8;
        let health_score = 100u8.saturating_sub(issue_penalty);

        Ok(SecurityAudit {
            total_entries: total,
            breached_count: breached,
            weak_count: weak,
            reused_count: reused,
            old_count: old,
            no_2fa_count: no_2fa,
            health_score,
            issues,
        })
    }
}
