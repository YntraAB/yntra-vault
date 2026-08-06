# Yntra Vault — Integration SDK & Architecture Reference

> Technical reference for the browser integration, vault file format, autotype engine, and release workflow.

---

## 1. Vault File Format (`.vdb`)

### Binary Layout

```
┌──────────────────────────────────┐
│  Magic: "YNTR" (4 bytes)        │
│  Version: u16 LE (2 bytes)      │
│  Flags: u16 LE (2 bytes)        │
│  Salt: [u8; 32] (32 bytes)      │
│  HMAC-SHA512: [u8; 64]          │
│  KDF Params Length: u32 LE      │
│  KDF Params (bincode)           │
│  Payload Length: u64 LE         │
│  ────────────────────────────── │
│  Encrypted Payload              │
└──────────────────────────────────┘
```

### Format Versions

| Version | Payload Encoding | Status |
|---------|-----------------|--------|
| `1` | bincode (positional, legacy) | Read-only — auto-upgraded on save |
| `2` | MessagePack (self-describing) | Current — all writes use v2 |

**Migration behavior**: Opening a v1 vault deserializes via bincode with legacy struct fallback, then re-saves as v2 MessagePack on next write. No user action required.

### Encryption Pipeline

```
Master Password
    │
    ▼
Argon2id (256 MB, 4 passes, 4 threads)
    │
    ▼
HKDF-SHA512 ──┬── Vault Key (XChaCha20-Poly1305)
               ├── Entry Key (AES-256-GCM)
               ├── HMAC Key (HMAC-SHA512)
               └── Search Key (trigram hashing)
```

**Decryption order**:
1. Parse header → extract salt + KDF params
2. Derive keys from password + salt
3. **Verify HMAC-SHA512** over encrypted payload (reject tampered files before decryption)
4. Decrypt outer layer (XChaCha20-Poly1305 → VaultData)
5. Per-entry fields decrypted on-demand (AES-256-GCM)

### Adding Fields to `Entry` (v2+ rules)

| Operation | Safe? | How |
|-----------|-------|-----|
| New optional field | ✅ | `#[serde(default)] pub field: Option<T>` |
| New required field | ❌ | Must be `Option<T>` with default |
| Rename field | ❌ | Use `#[serde(alias = "old")]` |
| Remove field | ⚠️ | `#[serde(skip_deserializing)]` first, remove in next major |
| Change field type | ❌ | Add new field, deprecate old |

---

## 2. Browser Integration (Native Messaging)

### Architecture

```
Browser Extension ◄──── stdin/stdout (4-byte length prefix) ────► Native Host
                                                                      │
                                                     Named Pipe / Unix Socket
                                                                      │
                                                              Tauri Desktop App
```

**Components**:
- **Native Host** (`src-core/src/bin/yntra-vault-native-host.rs`): Lightweight Rust binary registered as a Chrome/Firefox Native Messaging host.
- **IPC Server** (`src-core/src/vault/ipc_server.rs`): Runs inside the Tauri app, listens on `\\.\pipe\yntra-vault-ipc` (Windows) or `/tmp/yntra-vault-ipc.sock` (Unix).

### Security Model

1. **Parent Process Verification** (Windows):
   - Resolves PPID via Toolhelp32 Snapshots
   - Verifies parent executable matches an allowed browser (`chrome.exe`, `firefox.exe`, `msedge.exe`, `brave.exe`, `vivaldi.exe`, `arc.exe`)
   - Rejects execution from unknown parents

2. **Session Token Verification**:
   - Tauri app generates a cryptographic token on vault unlock
   - Token is wrapped via DPAPI (Windows) / Keychain (macOS)
   - All IPC requests must include a valid `session_token`
   - Token comparison uses constant-time equality (`subtle::ConstantTimeEq`)

3. **Message Size Limit**: All payloads capped at 1 MB

### SDK Request/Response Format

All payloads are JSON.

#### `get_credentials`

```json
// Request
{
  "action": "get_credentials",
  "domain": "github.com",
  "session_token": "a1b2c3d4..."
}

// Success
{
  "username": "user123",
  "password": "decryptedPassword",
  "email": "user@example.com"
}

// Errors
{"error": "Unauthorized parent process"}
{"error": "Invalid session token"}
{"error": "Vault is locked"}
{"error": "No matching credentials found"}
```

### Installation

The `install_browser_extension` Tauri command:
1. Writes a Native Messaging manifest JSON to the OS-specific location
2. On Windows: registers `HKCU\SOFTWARE\Google\Chrome\NativeMessagingHosts\com.yntra.vault`
3. On macOS: writes to `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`
4. On Linux: writes to `~/.config/google-chrome/NativeMessagingHosts/`

---

## 3. Autotype Engine

The autotype engine (`src-core/src/vault/autotype.rs`) types credentials into focused application fields using OS-level input simulation.

### Field Classification

The engine classifies focused elements by inspecting accessibility properties:

| Priority | Classification | Detection |
|----------|---------------|-----------|
| 1 | **TOTP/2FA** | Keywords: `code`, `token`, `totp`, `2fa`, `otp`, `mfa` |
| 2 | **Password** | `IsPassword == true`, or keywords: `password`, `lösenord`, `pass` |
| 3 | **Username** | Any visible, focusable edit control not matching above |

### Safety Guards

- **Window Lock**: Captures `target_hwnd` on start. If the foreground window changes mid-type, execution aborts immediately.
- **Focus Settle Polling**: If autotype is triggered while Yntra Vault is the active foreground window, the engine polls (max 15s) until the active window changes, then sleeps for the configured `autotypeSettleDelayMs` before starting to type.
- **Exclusion Filter**: Skips elements with names/classes containing `search`, `sök`, `find`, `chat`, `message`, `reply`, `comment`, `prompt`.
- **Auto-Focus**: If no input is focused, scans for the first valid edit field and focuses it (1s intervals, max 15s).
- **Memory Zeroization**: All sensitive autotype data is wrapped in `AutotypeGuard` which implements `ZeroizeOnDrop`.

### Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `autotypeCharDelayMs` | 15 | Delay between keystrokes (ms) |
| `autotypeFieldDelayMs` | 300 | Delay between fields (ms) |
| `autotypeSettleDelayMs` | 3000 | Delay after target fönster is focused before typing (ms) |
| `autotypeLaunchBrowser` | true | Open URL in browser before typing |

---

## 4. Passkey Authenticator (ES256)

### Cryptographic Scheme

- **Algorithm**: ECDSA over NIST P-256 (ES256)
- **Private Key Storage**: Encrypted per-entry using AES-256-GCM (same entry key as passwords)
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

The search system uses **trigram hashing** to enable fuzzy search without exposing plaintext index data.

1. Entry metadata is split into character trigrams
2. Each trigram is HMAC-hashed with a dedicated `search_key`
3. Hashed trigrams are stored in a `HashMap<[u8; 32], Vec<Uuid>>`
4. Query trigrams are hashed with the same key and matched against the index
5. Results require ≥80% trigram overlap (fuzzy matching)

---

## 6. Shamir Secret Sharing

Vault recovery supports splitting the master key into N shares where K are required to reconstruct (K-of-N threshold).

- **Field**: GF(256) with irreducible polynomial `x⁸ + x⁴ + x³ + x + 1`
- **Share Format**: `YNTRA-SHARE-{index}-{hex_data}`
- **Constant-Time**: All GF(256) arithmetic uses lookup tables to prevent timing side-channels

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
- Never commit directly — all updates via sync script

### Sync Command

```powershell
.\publish-public.ps1 -DestDir <path-to-public-repo> -Push
```

**Steps performed**:
1. Wipes destination (preserves `.git/`)
2. Copies `src/`, `src-core/`, `src-tauri/`, `public/`
3. Excludes `target/`, `node_modules/`, `.agents/`, dev `.md` files
4. Copies config files (`package.json`, `tsconfig.json`, etc.)
5. Renames `README-public.md` → `README.md`
6. Commits as `pysen00` and pushes to GitHub

---

## 8. IPC Command Reference (Tauri)

All commands are invoked from the React frontend via `@tauri-apps/api/core::invoke()`.

### Vault Lifecycle

| Command | Arguments | Returns |
|---------|-----------|---------|
| `list_vaults` | — | `VaultInfo[]` |
| `create_vault` | `name`, `password`, `path` | `VaultInfo` |
| `open_vault` | `path`, `password` | `VaultInfo` |
| `lock_vault` | — | `()` |
| `close_vault` | — | `()` |
| `change_master_password` | `current`, `new_password` | `()` |
| `get_vault_path` | — | `string` |
| `export_vault` | `destination` | `()` |

### Entry CRUD

| Command | Arguments | Returns |
|---------|-----------|---------|
| `list_entries` | — | `EntryPreview[]` |
| `get_entry` | `id` | `DecryptedEntry` |
| `add_entry` | `entry: NewEntry` | `string` (UUID) |
| `update_entry` | `id`, `entry: UpdateEntry` | `()` |
| `delete_entry` | `id` | `()` |
| `search_entries` | `query` | `EntryPreview[]` |

### Security & Tools

| Command | Arguments | Returns |
|---------|-----------|---------|
| `check_breach` | `password` | `BreachResult` |
| `security_audit` | — | `SecurityAudit` |
| `generate_password` | `options: GeneratorOptions` | `string` |
| `get_totp_code` | `secret` | `TotpCode` |
| `autotype` | `text`, `char_delay_ms`, `settle_delay_ms` | `()` |
| `run_smart_autotype` | `username`, `password`, `totp_secret`, `url`, `launch_browser`, `char_delay_ms`, `field_delay_ms` | `()` |
| `set_minimize_to_tray` | `enabled` | `()` |
| `install_browser_extension` | — | `()` |
