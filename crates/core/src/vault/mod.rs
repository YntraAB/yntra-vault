pub mod types;
pub mod format;
pub mod manager;
pub mod entry;
pub mod history;
pub mod search;
pub mod attachments;
pub mod trash;
pub mod tags;
pub mod audit;
pub mod rekey;
pub mod auth;
pub mod emergency;
pub mod importer;
pub mod import_export;

// Backwards-compatibility aliases and re-exports from services subsystem
pub use crate::services::autotype;
pub use crate::services::autofill as mobile_autofill;
pub use crate::services::sync;
pub use crate::services::autostart;

pub use types::*;
pub use manager::VaultManager;
pub use entry::{NewEntry, UpdateEntry, DecryptedEntry};
pub use trash::{TrashedEntryPreview, VaultStorageMetrics};
pub use emergency::{EmergencyKit, EmergencyShare};
pub use search::{generate_index_tokens, generate_trigrams, hash_trigram};
pub use autotype::{autotype_text, autotype_text_with_delay, run_smart_autotype, run_smart_autotype_with_delays};
pub use sync::{webdav_upload, webdav_download, webdav_download_bytes, decrypt_remote_vault_bytes, decrypt_remote_vault_bytes_checked, webdav_test_connection, webdav_get_etag, merge_vault_data, MergeStats, run_p2p_sync_listener, run_p2p_sync_client};
pub use autostart::{enable_autostart, disable_autostart, is_autostart_enabled};
pub use importer::*;
pub use mobile_autofill::*;
