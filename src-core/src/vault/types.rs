//! Vault data types
//!
//! All types that get serialized into the .vdb vault file.
//! Includes entry templates, password history, and audit metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::cipher::EncryptedBlob;

// ─── Entry Types ────────────────────────────────────────────────────────

/// A password entry — the core data unit.
/// Sensitive fields (password, totp_secret) are individually encrypted.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub id: Uuid,
    pub title: String,
    pub username: String,
    /// Encrypted with per-entry AES-256-GCM
    pub encrypted_password: EncryptedBlob,
    pub url: String,
    pub email: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub pinned: bool,
    /// TOTP secret — encrypted with per-entry key
    pub encrypted_totp_secret: Option<EncryptedBlob>,
    pub custom_fields: Vec<CustomField>,
    pub entry_type: EntryType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Password history — keeps last N password changes for rollback
    pub password_history: Vec<PasswordHistoryItem>,
    /// Breach detection status
    pub breach_status: BreachStatus,
    /// Password strength score (cached)
    pub strength_score: Option<StrengthScore>,
    /// When the password was last changed
    pub password_changed_at: DateTime<Utc>,
}

/// Lightweight entry preview for list views — no decryption needed.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntryPreview {
    pub id: Uuid,
    pub title: String,
    pub username: String,
    pub url: String,
    pub email: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub pinned: bool,
    pub has_totp: bool,
    pub entry_type: EntryType,
    pub updated_at: DateTime<Utc>,
    pub breach_status: BreachStatus,
    pub strength_score: Option<StrengthScore>,
    pub password_age_days: i64,
}

/// Pre-built entry templates for common account types.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum EntryType {
    Login,
    CreditCard,
    Identity,
    SecureNote,
    SshKey,
    ApiKey,
    WifiPassword,
    CryptoWallet,
    Custom,
}

impl Default for EntryType {
    fn default() -> Self {
        EntryType::Login
    }
}

/// Custom field with type information.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomField {
    pub id: Uuid,
    pub name: String,
    pub field_type: FieldType,
    /// Value is encrypted for sensitive field types
    pub value: String,
    pub sensitive: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum FieldType {
    Text,
    Password,
    Username,
    Email,
    Url,
    Phone,
    Date,
    Address,
    Notes,
    Totp,
    File,
}

// ─── Password History ───────────────────────────────────────────────────

/// Keeps old passwords so you can roll back if needed.
/// Each entry keeps the last 10 password changes.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PasswordHistoryItem {
    pub encrypted_password: EncryptedBlob,
    pub changed_at: DateTime<Utc>,
}

pub const MAX_PASSWORD_HISTORY: usize = 10;

// ─── Breach Detection Types ────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BreachStatus {
    /// Not yet checked
    Unknown,
    /// Currently checking
    Checking,
    /// Not found in any breaches
    Safe { checked_at: DateTime<Utc> },
    /// Found in breach database
    Breached {
        breach_count: u64,
        checked_at: DateTime<Utc>,
    },
    /// Check failed (offline, rate limited, etc)
    Error { message: String },
}

impl Default for BreachStatus {
    fn default() -> Self {
        BreachStatus::Unknown
    }
}

// ─── Password Strength Types ───────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StrengthScore {
    /// Entropy in bits
    pub entropy_bits: f64,
    /// Human-readable crack time estimate
    pub crack_time: String,
    /// Overall classification
    pub level: StrengthLevel,
    /// Specific issues found
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StrengthLevel {
    Critical,  // < 25 bits
    Weak,      // 25-50 bits
    Fair,      // 50-75 bits
    Strong,    // 75-100 bits
    Excellent, // 100+ bits
}

// ─── Tag ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub icon: String,
}

// ─── Vault Metadata ────────────────────────────────────────────────────

/// Vault metadata stored in the file header (unencrypted).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultMetadata {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub entry_count: usize,
    pub version: u16,
}

/// The full decrypted vault contents.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultData {
    pub metadata: VaultMetadata,
    pub entries: Vec<Entry>,
    pub tags: Vec<Tag>,
    /// Deleted entries kept for 30 days
    pub trash: Vec<TrashedEntry>,
}

/// Entry in the trash — auto-deleted after 30 days.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrashedEntry {
    pub entry: Entry,
    pub deleted_at: DateTime<Utc>,
}

/// Vault info for the vault selection screen (no decryption needed).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultInfo {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub entry_count: usize,
    pub last_opened: Option<DateTime<Utc>>,
}

// ─── Security Audit Types ───────────────────────────────────────────────

/// Overall security health report for the vault.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityAudit {
    pub total_entries: usize,
    pub breached_count: usize,
    pub weak_count: usize,
    pub reused_count: usize,
    pub old_count: usize, // > 90 days unchanged
    pub no_2fa_count: usize,
    pub health_score: u8, // 0-100
    pub issues: Vec<SecurityIssue>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityIssue {
    pub entry_id: Uuid,
    pub entry_title: String,
    pub issue_type: IssueType,
    pub severity: IssueSeverity,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum IssueType {
    Breached,
    WeakPassword,
    ReusedPassword,
    OldPassword,
    Missing2FA,
    ShortPassword,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Info,
    Warning,
    Critical,
}

