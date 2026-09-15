# Yntra Vault Database Format Specification (`.vdb` Format Version 4)

**Document Version**: 1.0  
**Target Format Version**: 4  
**Classification**: Public Specification  
**Reference Codebase**: [crates/core/src/vault/format.rs](../../crates/core/src/vault/format.rs)

---

## 1. Overview & Architectural Principles

The `.vdb` (Yntra Vault Database) file format is a binary container designed for high-security, offline-first password management. It implements a multi-layer authenticated encryption pipeline designed to remain resilient against key-exfiltration, ciphertext swapping, downgrade attacks, and offline brute-force attempts.

### Key Security Properties
- **Authenticated Encryption with Associated Data (AEAD)**: The entire unencrypted file header is bound as Additional Authenticated Data (AAD) into the XChaCha20-Poly1305 outer encryption stream. Any modification to the salt, flags, format version, or Argon2id parameters invalidates the Poly1305 tag and aborts decryption before deserialization.
- **Domain-Isolated Per-Entry Encryption**: Sensitive fields within entries (passwords, TOTP secrets, passkey private keys, file attachments) are encrypted individually using a secondary key layer with per-field AAD domain isolation (`entry:{entry_id}:{field_scope}`).
- **Hardened Memory Safety**: Decrypted intermediate buffers must be zeroed using volatile writes (`Zeroize`) and allocated within locked, canary-guarded memory (`LockedBuffer`).
- **Memory-Hard KDF**: Master keys are stretched using Argon2id (default 256 MB RAM, 4 passes, 4 threads) with minimum security validation bounds enforced on load.

---

## 2. Binary File Layout

A `.vdb` file consists of an unencrypted header, optional embedded authentication metadata containers, and an encrypted MessagePack payload. All multi-byte integers are encoded in **Little-Endian (LE)** byte order.

```
┌────────────────────────────────────────────────────────────────────────┐
│ Magic Bytes: "YNTR" (4 bytes)                                           │
├────────────────────────────────────────────────────────────────────────┤
│ Format Version: u16 LE (2 bytes) -> Current: 4                        │
├────────────────────────────────────────────────────────────────────────┤
│ Flags: u16 LE (2 bytes)                                                │
│   - 0x0001 = FLAG_HAS_BIOMETRIC                                        │
│   - 0x0002 = FLAG_HAS_HARDWARE_2FA                                     │
├────────────────────────────────────────────────────────────────────────┤
│ Salt: [u8; 32] (32 bytes)                                              │
├────────────────────────────────────────────────────────────────────────┤
│ [Legacy Outer HMAC: 64 bytes (Included ONLY if Version <= 2)]          │
├────────────────────────────────────────────────────────────────────────┤
│ KDF Parameters Length: u32 LE (4 bytes)                                │
├────────────────────────────────────────────────────────────────────────┤
│ KDF Parameters Payload (Bincode serialized struct KdfParams)           │
├────────────────────────────────────────────────────────────────────────┤
│ [Biometric Block Length: u32 LE] + Payload (If FLAG_HAS_BIOMETRIC)     │
├────────────────────────────────────────────────────────────────────────┤
│ [Hardware 2FA Block Length: u32 LE] + Payload (If FLAG_HAS_HARDWARE_2FA)│
├────────────────────────────────────────────────────────────────────────┤
│ Payload Length: u64 LE (8 bytes)                                       │
├────────────────────────────────────────────────────────────────────────┤
│ Encrypted Payload (XChaCha20-Poly1305 Ciphertext + 16-byte Poly1305 Tag)│
└────────────────────────────────────────────────────────────────────────┘
```

### Detailed Field Descriptions

| Field | Type | Size | Description |
|---|---|---|---|
| `magic` | `[u8; 4]` | 4 bytes | Literal ASCII string `b"YNTR"`. |
| `version` | `u16` | 2 bytes | Format version (current: `4`). |
| `flags` | `u16` | 2 bytes | Bitmask flags (`0x0001` Biometrics enrolled, `0x0002` Hardware 2FA enabled). |
| `salt` | `[u8; 32]` | 32 bytes | Cryptographically random salt generated via OS CPRNG (`rand::rng`). |
| `kdf_len` | `u32` | 4 bytes | Byte length of the serialized `KdfParams` structure. |
| `kdf_params` | Byte array | Variable | `bincode` serialized `KdfParams` struct containing memory, pass count, parallelism, and output length. |
| `bio_block` | Optional | Variable | Present only if `flags & 0x0001 != 0`. Length-prefixed `bincode` container holding biometric key wrappers. |
| `hw2fa_block` | Optional | Variable | Present only if `flags & 0x0002 != 0`. Length-prefixed `bincode` container holding hardware token parameters. |
| `payload_len` | `u64` | 8 bytes | Byte length of the outer encrypted MessagePack payload. |
| `encrypted_payload` | Byte array | Variable | XChaCha20-Poly1305 ciphertext + 16-byte Poly1305 tag over the outer `VaultData` payload. |

---

## 3. Cryptographic Pipeline & Key Hierarchy

```
Master Password [+ Optional Keyfile]
    │
    ▼
Argon2id Stretcher (Salt, 256 MB, 4 Passes, 4 Threads)
    │
    ▼ 64-byte Master Key
HKDF-SHA512 Stretcher
    ├── Info: "yntra-vault-encryption-key-v1"        ──► Vault Key (XChaCha20-Poly1305 outer payload)
    ├── Info: "yntra-vault-entry-encryption-key-v1"  ──► Master Entry Key (Derives per-entry keys)
    ├── Info: "yntra-vault-hmac-integrity-key-v1"    ──► HMAC Key (Legacy verification & P2P handshake)
    └── Info: "yntra-vault-trigram-search-key-v1"    ──► Search Key (Trigram HMAC search index)
```

### 1. Master Key Derivation (Argon2id)
The raw master password and optional keyfile bytes are processed through Argon2id:
- **Salt**: 32 bytes (stored unencrypted in header)
- **Memory Cost**: Default `262,144` KiB (256 MB), Minimum enforcing `8,192` KiB
- **Time Cost**: Default `4` iterations, Minimum enforcing `1`
- **Parallelism**: Default `4` threads, Minimum enforcing `1`
- **Output Length**: `64` bytes

### 2. Subkey Derivation (HKDF-SHA512)
The 64-byte Argon2id output acts as pseudo-random keying material (`PRK`) for HKDF-SHA512 expansion without an additional salt:
- **`VaultKey`**: `HKDF-Expand(PRK, info="yntra-vault-encryption-key-v1", length=32)`
- **`EntryKey`**: `HKDF-Expand(PRK, info="yntra-vault-entry-encryption-key-v1", length=32)`
- **`HmacKey`**: `HKDF-Expand(PRK, info="yntra-vault-hmac-integrity-key-v1", length=64)`
- **`PerEntryKey`**: `HKDF-Expand(EntryKey, info="yntra-vault-per-entry-key-v1-" || entry_id, length=32)`

---

## 4. AEAD AAD Header Binding & Field Domain Isolation

### Outer Header AAD Construction
To prevent ciphertext substitution or parameter tampering, the outer layer computes an AAD byte vector before decrypting or encrypting:

$$\text{AAD} = \text{Magic (4B)} \mathbin{\Vert} \text{Version (2B)} \mathbin{\Vert} \text{Flags (2B)} \mathbin{\Vert} \text{Salt (32B)} \mathbin{\Vert} \text{KdfLen (4B)} \mathbin{\Vert} \text{KdfBytes}$$

Decryption calls `XChaCha20Poly1305::decrypt(&nonce, Payload, AAD)`. If any byte in the header is altered, decryption fails with an authentication error.

### Inner Field Scope AAD Isolation
Per-entry sensitive fields are encrypted using `EntryKey` with XChaCha20-Poly1305 and domain-isolated AAD:

$$\text{Field AAD} = \text{"entry:"} \mathbin{\Vert} \text{entry\_id (UUID string)} \mathbin{\Vert} \text{":"} \mathbin{\Vert} \text{field\_scope}$$

**Supported Field Scopes**:
- `password`: Main entry password
- `totp`: TOTP secret key string
- `passkey`: ES256 Passkey private key bytes
- `history`: Historical password entry
- `attachment:{attachment_id}`: Encrypted file attachment blob
- `custom:{field_id}:{field_name}`: Sensitive custom field

---

## 5. Serialized Payload Structure (`VaultData`)

The decrypted outer payload is decoded using **MessagePack** (`rmp-serde`). The outer structure is defined as follows:

```rust
pub struct VaultData {
    pub metadata: VaultMetadata,
    pub entries: Vec<Entry>,
    pub tags: Vec<Tag>,
    pub trash: Vec<TrashedEntry>,
    pub settings: VaultSettings,
}

pub struct VaultSettings {
    pub webdav: WebdavConfig,
    pub emergency_kit_audit: Option<EmergencyKitAudit>,
}

pub struct EmergencyKitAudit {
    pub active_fingerprint: String,
    pub last_generated_at: DateTime<Utc>,
    pub generation_count: u32,
    pub history: Vec<EmergencyKitAuditEntry>,
}
```

### EncryptedBlob Encapsulation Structure
Sensitive inner fields use `EncryptedBlob`:

```rust
pub struct EncryptedBlob {
    pub nonce: [u8; 24],      // XChaCha20 192-bit random nonce
    pub ciphertext: Vec<u8>,  // Ciphertext
    pub tag: [u8; 16],        // Poly1305 128-bit authentication tag
}
```

---

## 6. Version History & Compatibility Matrix

| Version | Payload Format | Integrity Mechanism | Migration Behavior |
|---|---|---|---|
| `1` | `bincode` | Outer HMAC-SHA512 | Read-only; auto-upgraded to v4 on write. |
| `2` | MessagePack | Outer HMAC-SHA512 | Read-only; auto-upgraded to v4 on write. |
| `3` | MessagePack | Header AAD Binding | Supported legacy format; upgraded on save. |
| `4` | MessagePack | Header AAD Binding + Bio/HW2FA Containers | **Current standard format.** |

---

## 7. Verification & Implementation Checklist

When implementing an independent parser or validator for `.vdb` files:
1. ✅ **Validate Magic Bytes**: Ensure file starts with `YNTR`.
2. ✅ **Check Format Version**: Reject files with version $> 4$.
3. ✅ **Enforce KDF Bounds**: Reject parameters outside secure operational bounds (`8_192 <= memory_kb <= 1_048_576`, `1 <= iterations <= 64`, `1 <= parallelism <= 32`).
4. ✅ **Construct Header AAD**: Assemble canonical header bytes before invoking AEAD decrypt.
5. ✅ **Zero Sensitive Memory**: Immediately clear derived subkeys and decrypted fields using volatile zeroing (`Zeroize`).

---

## 8. Storage Compaction & Recovery Extensions

### Storage Footprint & Compaction (`compact_vault`)
Vault storage lifecycle maintains optimal binary footprint via:
1. **Trash Compaction**: Automatic purge of tombstones older than 30 days (`purge_expired_trash(30)`).
2. **Search Index Garbage Collection**: Prunes empty trigram buckets and rebuilds inverted token indexes on save.
3. **Storage Analytics**: `VaultStorageMetrics` provides real-time counts and byte footprints of active entries, attachments, and soft-deleted records.

### Master Password Emergency Recovery Kit
The emergency recovery protocol splits the vault's master password into three shares using 2-of-3 Shamir's Secret Sharing over $\text{GF}(2^8)$:
- Any 2 shares are mathematically sufficient to reconstruct the master password.
- Any 1 share alone reveals zero information regarding the secret.
- Shares are protected with a SHA-256 fingerprint check for visual self-verification.
- **Pre-Generation Master Password Verification**: `VaultManager::generate_emergency_kit` validates the candidate master password against active session keys via constant-time comparison before computing shares.
- **Encrypted In-Vault Audit Trail**: Generation timestamps, share fingerprints, and reset actions are persisted inside `VaultSettings.emergency_kit_audit` within the encrypted `.vdb` payload.

### 2FA Recovery Codes Storage Invariant
In `.vdb` payloads, 2FA backup codes are persisted as a sensitive custom field with canonical name `2FA Recovery Codes` (`field_type: Password`). The persistence layer strips duplicate or legacy variations (`2FA Recovery Cod`, `Recovery Codes`, etc.) during entry save/update operations, ensuring exactly one canonical record is maintained per entry.
