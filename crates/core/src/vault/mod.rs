pub mod types;
pub mod validation;
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
pub mod storage;
pub mod usb;
pub mod protection;
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
pub use sync::{webdav_upload, webdav_download, webdav_download_bytes, decrypt_remote_vault_bytes, decrypt_remote_vault_bytes_checked, webdav_test_connection, webdav_get_etag, merge_vault_data, MergeStats, run_p2p_sync_listener, run_p2p_sync_client, compute_p2p_discovery_id, get_local_lan_ip, broadcast_discovery_beacon, listen_discovery_beacon, DEFAULT_P2P_PORT, DEFAULT_DISCOVERY_PORT, DEFAULT_PAIRING_PORT, PairingStats, generate_pairing_code, normalize_pairing_code, compute_pairing_beacon_id, broadcast_pairing_beacon, listen_pairing_beacon, run_p2p_pairing_host, run_p2p_pairing_host_with_device_and_cancel, run_p2p_pairing_client, run_p2p_pairing_client_with_device, ClientPairingMode};
pub use autostart::{enable_autostart, disable_autostart, is_autostart_enabled};
pub use importer::*;
pub use mobile_autofill::*;
