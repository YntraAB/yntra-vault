# Yntra Vault — Integration SDK & Architecture Reference

> Technical reference for the browser integration, vault file format, autotype engine, and release workflow.

---

## 1. Vault File Format (`.vdb`)

### Binary Layout

```
┌─────────────────────────────────────────┐
│ Magic: "YNTR" (4 bytes)                 │
│ Version: u16 LE (2 bytes) -> 4          │
│ Flags: u16 LE (2 bytes)                 │  (0x0001 = Biometric, 0x0002 = HW2FA)
│ Salt: [u8; 32] (32 bytes)               │
│ [HMAC: 64 bytes (v1/v2 legacy only)]    │
│ KDF Params Length: u32 LE (4 bytes)     │  (Header bound as AAD)
│ KDF Params (bincode)                    │
│ [Biometric Block Length + Payload]      │  (Optional: if FLAG_HAS_BIOMETRIC)
│ [Hardware 2FA Block Length + Payload]   │  (Optional: if FLAG_HAS_HARDWARE_2FA)
│ Payload Length: u64 LE (8 bytes)        │
│ ─────────────────────────────────────── │
│ Encrypted Payload (MessagePack)         │  (XChaCha20-Poly1305 + Tag)
└─────────────────────────────────────────┘
```

### Format Versions

| Version | Payload Encoding | Status |
|---------|-----------------|--------|
| `1` | bincode (positional, legacy) | Read-only — auto-upgraded on save |
| `2` | MessagePack + Outer HMAC | Read-only — auto-upgraded on save |
| `3` | MessagePack + AAD Header Binding | Supported legacy format |
| `4` | MessagePack + AAD Header Binding + Biometric/HW2FA Header | Current — single-pass SOTA format |

**Migration behavior**: Opening a v1, v2, or v3 vault deserializes legacy data and verifies legacy signatures where applicable, then re-saves as v4 MessagePack with AAD header binding on next write. No user action required.

### Encryption Pipeline

```
Master Password [+ Optional Keyfile]
    │
    ▼
Argon2id (256 MB, 4 passes, 4 threads)
    │
    ▼
HKDF-SHA512 ──┬── Vault Key (XChaCha20-Poly1305 + Header AAD)
               ├── Entry Key (XChaCha20-Poly1305 / AES-256-GCM)
               ├── HMAC Key (HMAC-SHA512 / P2P Auth)
               └── Search Key (trigram HMAC hashing)
```

**Decryption order**:
1. Parse header → extract salt + KDF params (validate minimum KDF parameter bounds)
2. Derive keys from password/keyfile + salt
3. **Verify Authenticated Header & Payload** via single-pass XChaCha20-Poly1305 using header AAD (reject tampered files; legacy v1/v2 files verify outer HMAC first)
4. Decrypt outer layer (XChaCha20-Poly1305 → VaultData)
5. Per-entry fields decrypted on-demand (XChaCha20-Poly1305 / AES-256-GCM)

### Adding Fields to `Entry` (v2+ rules)

| Operation | Safe? | How |
|-----------|-------|-----|
| New optional field | ✅ | `#[serde(default)] pub field: Option<T>` |
| New required field | ❌ | Must be `Option<T>` with default |
| Rename field | ❌ | Use `#[serde(alias = "old")]` |
| Remove field | ⚠️ | `#[serde(skip_deserializing)]` first, remove in next major |
| Change field type | ❌ | Add new field, deprecate old |

---

## 2. Browser Integration & Architecture [Specification / Out of Scope]

> [!NOTE]
> Yntra Vault operates strictly offline without background network services or browser extension processes. All application and web form auto-filling is performed directly via the OS-level Autotype Engine. The specification below outlines the conceptual architecture for browser extensions.

### Conceptual Architecture

```
Browser Extension ◄──── stdin/stdout (4-byte length prefix) ────► Native Host
                                                                       │
                                                     Named Pipe / Unix Socket
                                                                       │
                                                               Tauri Desktop App
```

**Components**:
- **Native Host**: Lightweight executable registered as a Chrome/Firefox Native Messaging host.
- **IPC Server**: Runs inside the desktop app, listens on `\\.\pipe\yntra-vault-ipc` (Windows) or `/tmp/yntra-vault-ipc.sock` (Unix).

### Security Model

1. **Parent Process Verification** (Windows):
   - Resolves PPID via Toolhelp32 Snapshots
   - Verifies parent executable matches an allowed browser (`chrome.exe`, `firefox.exe`, `msedge.exe`, `brave.exe`, `vivaldi.exe`, `arc.exe`)
   - Rejects execution from unknown parents

2. **Session Token Verification**:
   - App generates a cryptographic token on vault unlock
   - Token is wrapped via DPAPI (Windows) / Keychain (macOS)
   - All IPC requests must include a valid `session_token`
   - Token comparison uses constant-time equality (`subtle::ConstantTimeEq`)

3. **Message Size Limit**: All payloads capped at 1 MB

---

## 3. Autotype Engine

The autotype engine (`src-core/src/vault/autotype.rs`) types credentials into focused application fields using OS-level input simulation.

### Field Classification

The engine classifies focused elements by inspecting accessibility properties:

| Priority | Classification | Detection |
|----------|---------------|-----------|
| 1 | **TOTP/2FA** | Keywords: `code`, `token`, `totp`, `2fa`, `otp`, `mfa`, `verification`, `kod`, `säkerhet`, `security` |
| 2 | **Password** | `IsPassword == true`, or keywords: `password`, `lösenord`, `pass` |
| 3 | **Username** | Any visible, focusable edit control not matching above |

### Safety Guards

- **Window Lock**: Captures `target_hwnd` on start. If the foreground window changes mid-type, execution aborts immediately.
- **Focus Settle Polling**: If autotype is triggered while Yntra Vault is active, the engine polls (max 15s) until the active window changes, then sleeps for `settle_delay_ms` before typing.
- **Exclusion Filter**: Skips elements with names/classes containing `search`, `sök`, `find`, `chat`, `message`, `reply`, `comment`, `prompt`, `filter`, `query`, `ask`, `fråga`, `gpt`, `copilot`.
- **Auto-Focus**: If no input is focused, scans for the first valid edit field and focuses it (1s intervals, max 15s).
- **Memory Zeroization**: Sensitive credentials are automatically wrapped in `AutotypeGuard` implementing `ZeroizeOnDrop` via `Zeroizing<String>`.

---

## 4. Passkey Authenticator (ES256)

### Cryptographic Scheme

- **Algorithm**: ECDSA over NIST P-256 (ES256)
- **Private Key Storage**: Encrypted per-entry using AES-256-GCM / XChaCha20-Poly1305
- **Public Key Format**: SEC1 uncompressed (65 bytes)

### API

```rust
// Generate
manager.add_entry(NewEntry { generate_passkey: Some(true), .. })

// Toggle on existing entry
manager.update_entry(id, UpdateEntry { passkey_action: Some("generate"), .. })
manager.update_entry(id, UpdateEntry { passkey_action: Some("remove"), .. })

// Check status
let entry = manager.get_entry(id);
entry.has_passkey      // bool
entry.passkey_public_key  // Option<Vec<u8>>
```

---

## 5. Encrypted Search (Zero-Disclosure)

The search system uses **trigram HMAC hashing** to enable fuzzy search without exposing plaintext index data.

1. Entry metadata (title, username, url, email, tags) is split into character trigrams
2. Each trigram is HMAC-SHA256 hashed with a dedicated `search_key` and truncated to 8 bytes (`[u8; 8]`)
3. Hashed trigrams are stored in a `HashMap<[u8; 8], Vec<Uuid>>`
4. Query trigrams are hashed with the same key and matched against the index
5. Results require ≥80% trigram overlap (fuzzy matching)

---

## 6. Shamir Secret Sharing

Vault recovery supports splitting a master password hash into 3 shares where any 2 are required to reconstruct (2-of-3 threshold).

- **Field**: GF(256) with irreducible polynomial `x⁸ + x⁴ + x³ + x + 1` (0x11b)
- **Share Format**: `SL-SHARE1-{hex}`, `SL-SHARE2-{hex}`, `SL-SHARE3-{hex}`
- **Scheme**: Password is hashed with SHA-256 to a 32-byte secret before splitting; reconstruction recovers the SHA-256 hash to verify identity.

---

## 7. Dual-Repository Release Workflow

```
yntra-vault-private ──── publish-public.ps1 ────► yntra-vault (public)
```

### Private Repository (`yntra-vault-private`)
- Full source + `.agents/` + dev docs + git history
- Commit normally with standard git

### Public Repository (`yntra-vault`)
- Clean source only — no `.agents/`, no dev `.md` files, no build artifacts
- All updates via sync script `publish-public.ps1`

---

## 8. IPC Command Reference (Tauri)

All commands are invoked from the React frontend via `@tauri-apps/api/core::invoke()`.

### Vault Lifecycle & Authentication

| Command | Arguments | Returns |
|---------|-----------|---------|
| `get_vault_info` | — | `Option<VaultInfo>` |
| `create_vault` | `name`, `password`, `path`, `key_file_path?` | `VaultInfo` |
| `open_vault` | `path`, `password`, `key_file_path?` | `VaultInfo` |
| `lock_vault` | — | `()` |
| `change_master_password` | `current`, `new_password`, `current_key_file?`, `new_key_file?` | `()` |
| `get_vault_path` | — | `string` |
| `generate_key_file` | `path` | `()` |

### Biometrics (Windows Hello / Touch ID)

| Command | Arguments | Returns |
|---------|-----------|---------|
| `check_biometric_available` | — | `BiometricInfo` |
| `is_biometric_enabled` | `path` | `bool` |
| `unlock_vault_biometric` | `path` | `VaultInfo` |
| `enable_biometric` | — | `()` |
| `disable_biometric` | — | `()` |

### Hardware 2FA / YubiKey

| Command | Arguments | Returns |
|---------|-----------|---------|
| `check_hardware2fa_available` | — | `Hardware2FaInfo` |
| `list_hardware_keys` | — | `HardwareKeyInfo[]` |
| `is_hardware2fa_enabled` | `path` | `bool` |
| `open_vault_with_hardware2fa` | `path`, `password`, `key_file_path?`, `hardware_response` | `VaultInfo` |
| `perform_hardware2fa_challenge` | `protocol`, `challenge?` | `Vec<u8>` |
| `enable_hardware2fa` | `protocol`, `key_name`, `hardware_response` | `()` |
| `disable_hardware2fa` | — | `()` |

### Entry & Trash CRUD

| Command | Arguments | Returns |
|---------|-----------|---------|
| `list_entries` | — | `EntryPreview[]` |
| `get_entry` | `id` | `DecryptedEntry` |
| `add_entry` | `entry: NewEntry` | `string` (UUID) |
| `update_entry` | `id`, `update: UpdateEntry` | `()` |
| `delete_entry` | `id` | `()` |
| `toggle_favorite` | `id` | `bool` |
| `toggle_pin` | `id` | `bool` |
| `search_entries` | `query` | `EntryPreview[]` |
| `list_trash` | — | `TrashedEntryPreview[]` |
| `restore_from_trash` | `id` | `()` |
| `permanent_delete` | `id` | `()` |
| `empty_trash` | — | `()` |

### Attachments

| Command | Arguments | Returns |
|---------|-----------|---------|
| `get_attachment_data` | `entry_id`, `attachment_id` | `Vec<u8>` |
| `add_attachment` | `entry_id`, `name`, `mime_type`, `data` | `AttachmentInfo` |
| `delete_attachment` | `entry_id`, `attachment_id` | `()` |

### Tags & Password History

| Command | Arguments | Returns |
|---------|-----------|---------|
| `get_tags` | — | `Tag[]` |
| `add_tag` | `name`, `color`, `icon` | `string` (UUID) |
| `update_tag` | `id`, `name`, `color`, `icon` | `()` |
| `delete_tag` | `id` | `()` |
| `get_password_history` | `entry_id` | `DecryptedHistoryItem[]` |

### Security & Tools

| Command | Arguments | Returns |
|---------|-----------|---------|
| `check_password_breach` | `password` | `BreachResult` |
| `analyze_password_strength` | `password` | `StrengthScore` |
| `security_audit` | — | `SecurityAudit` |
| `generate_password` | `options: GeneratorOptions` | `string` |
| `generate_password_default` | — | `string` |
| `generate_totp` | `secret` | `TotpCode` |
| `generate_totp_with_config` | `config: TotpConfig` | `TotpCode` |
| `parse_otpauth_uri` | `uri` | `TotpConfig` |
| `autotype` | `text`, `char_delay_ms`, `settle_delay_ms` | `()` |
| `run_smart_autotype` | `username`, `password`, `totp_secret`, `url`, `launch_browser`, `char_delay_ms`, `field_delay_ms` | `()` |
| `enable_autostart` | — | `()` |
| `disable_autostart` | — | `()` |
| `is_autostart_enabled` | — | `bool` |
| `set_minimize_to_tray` | `enabled` | `()` |
| `check_vault_file_exists` | `path` | `bool` |
| `show_in_explorer` | `path` | `()` |

### Synchronization (WebDAV & P2P)

| Command | Arguments | Returns |
|---------|-----------|---------|
| `webdav_test_connection` | `url`, `username`, `password?` | `()` |
| `webdav_upload` | `url`, `username`, `password?`, `db_path`, `if_match_etag?` | `Option<string>` |
| `webdav_download` | `url`, `username`, `password?`, `dest_db_path` | `()` |
| `webdav_sync` | `url`, `username`, `password?` | `MergeStats` |
| `run_p2p_sync_listener` | `listen_addr`, `db_path` | `()` |
| `run_p2p_sync_client` | `server_addr`, `db_path` | `()` |

### Shamir Recovery & Import / Export

| Command | Arguments | Returns |
|---------|-----------|---------|
| `split_master_password` | `password` | `Vec<string>` |
| `reconstruct_master_password_hash` | `share_a`, `share_b` | `string` (hex) |
| `export_vault` | `dest_path` | `()` |
| `export_vault_csv` | `dest_path` | `()` |
| `export_vault_json` | `dest_path` | `()` |
| `parse_import_file` | `file_path`, `format?` | `ImportPreviewResult` |
| `parse_import_content` | `content`, `format?` | `ImportPreviewResult` |
| `import_entries` | `entries`, `duplicate_strategy` | `usize` |
