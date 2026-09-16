# Changelog

All notable changes to Yntra Vault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.7] - 2026-09-15

### Added
- **Unified Trusted Devices Architecture**:
  - Implemented persistent device pairing registry in `VaultSettings.trusted_devices` with device UUID, human-readable name, OS badge, hardware type, and sync timestamps.
  - Added mutual device metadata exchange during 6-digit PIN handshake (`DeviceInfo`, `resolve_local_device_info`).
  - Added host-side device session revocation (`revoke_trusted_device`), actively expelling unapproved devices from the encrypted `.vdb` settings payload.
  - Added cryptographic revocation rejection signal (`P2P_REVOKED_SIG`), immediately closing connections from revoked devices and prompting the client to re-pair.
  - Added continuous, code-free background auto-synchronization for paired devices on local Wi-Fi.
- **Zero-Knowledge UDP Discovery Beacon (Port 5323)**:
  - Added zero-knowledge discovery beacon exchange via BLAKE3 keyed hash tokens (`compute_pairing_beacon_id`, `compute_p2p_discovery_id`) for automatic LAN discovery without manual IP entry.
- **Zero-Vault Adopt Flow on New Clients**:
  - Added automatic standard vault file initialization (`Documents/YntraVault/yntra-vault.vdb`) when pairing from an uninitialized client instance, adopting remote salt and entries seamlessly.
- **Trusted Devices Management UI**:
  - Added trusted devices section in Settings > Backup (`BackupTab.tsx`) with device type icons, OS labels, pairing dates, last sync timestamps, and per-device disconnect ("Koppla från") action.
  - Added single unified toggle for background Wi-Fi synchronization.

### Changed
- **6-Digit PIN Pairing Wizard UX**:
  - Upgraded PIN input in `DevicePairingWizard.tsx` to 3+3 triplet layout with clipboard paste (Ctrl+V) distribution and Enter key submission.
  - Replaced native browser checkboxes with standardized `<Toggle />` components across pairing and backup settings.
  - Refined action button margins and text container padding to eliminate viewport clipping.

### Security
- **P2P Transit AEAD Encryption (Defense-in-Depth)**:
  - Encrypted all database network payloads over TCP with XChaCha20-Poly1305 AEAD (`P2P_TRANSIT_AAD`) using `subkeys.vault_key`, maintaining transparent backward compatibility for unencrypted legacy payloads.
- **Zero-Knowledge Salt Commitment (P2P Handshake)**:
  - Replaced plaintext Argon2id root salt transmission over TCP with a 32-byte keyed BLAKE3 commitment (`compute_salt_commitment`), verifying vault compatibility early without disclosing the root salt.
- **Argon2id-Hardened Pairing Discovery Beacon**:
  - Replaced fast un-salted BLAKE3 PIN hashing with Argon2id-derived pairing subkeys (`compute_pairing_beacon_id_from_subkeys`, `compute_pairing_beacon_id`), preventing LAN eavesdroppers from conducting offline dictionary attacks against 6-digit PINs.
- **Encrypted Host Root Salt in Pairing**:
  - Encrypted the host pairing response payload (`PairingHostPayload`) using XChaCha20-Poly1305 AEAD (`PAIRING_AAD_HOST`) keyed with pairing subkeys, eliminating plaintext Argon2id salt transmission during initial device pairing.
- **Device Revocation & Nil-UUID Rejection**:
  - Enforced strict revocation checks rejecting `Uuid::nil()` or unlisted client devices with `P2P_REVOKED_SIG` when trusted devices are configured. Bound `client_device_uuid` directly into the client's mutual HMAC signature.
- **Volatile Memory Zeroization in P2P & Pairing**:
  - Wrapped all decrypted and merged database buffers in `zeroize::Zeroizing<Vec<u8>>` across `crates/core/src/services/sync/mod.rs` and `crates/core/src/services/sync/pairing.rs`.
- **Frontend Ephemeral Password Zeroing**:
  - Added unconditional state zeroization (`setPassword('')`, `setInputDigits(...)`) on modal close, completion, and unmount in `DevicePairingWizard.tsx`.
- **LAN Discovery Self-Echo Suppression**:
  - Added loopback and local LAN IP filtering in `scan_p2p_discovery` (`src-tauri/src/commands/sync.rs`), preventing desktop instances from discovering and attempting to sync with their own listeners.
- **P2P Handshake Timing and Asymmetry Hardening**:
  - Replaced dynamic length rejection responses with uniform 64-byte `P2P_AUTH_FAILED_SIG` and constant-time verification (`subtle::ConstantTimeEq`), preventing client panics and timing side-channels.
- **Root Salt Mismatch Enforcement**:
  - Enforced mutual root salt checks (`P2P_SALT_MISMATCH_MARKER`) early in the handshake phase before decrypting or processing remote payloads.
- **Constant-Time Hardware 2FA Comparison**:
  - Replaced non-constant-time byte comparison in `enable_hardware2fa_with_password()` with `subtle::ConstantTimeEq` (`ct_eq`), closing a timing side-channel during Hardware 2FA enrollment.
- **Session Token Path & Storage Hardening**:
  - Unified Windows session token location between CLI (`yntra-cli`) and core (`yntra-crypto`) to `%LOCALAPPDATA%/Yntra Vault/session.token`.
  - Removed plaintext fallback to ensure session tokens are strictly encrypted via App-Bound DPAPI / BLAKE3.
- **In-Memory Transit Secret Scrubbing**:
  - Wired `clearSessionSecrets()` to the `vault-connection-lost` event in `AuthContext`, ensuring transit import and sync credentials in volatile memory are immediately wiped on disconnect.
- **Key File POSIX Permission Hardening**:
  - Enforced POSIX file mode `0o600` (`read/write` owner-only) and filesystem sync (`sync_all()`) on newly generated key files in `rekey.rs`.
- **KDF Lower Bound Alignment**:
  - Normalized Argon2id lower resource bounds to 64 MB (`65_536 KB`) and 2 iterations across core validation logic, architecture specifications, and agent guidelines.

### Fixed
- **Host Device Authorization Fail-Closed Enforcement**:
  - Replaced silent error absorption (`if let Ok(...)`) during local vault read/decryption in P2P handshake with strict error propagation (`SyncError`), ensuring connections fail-closed if the trusted devices registry cannot be verified.
- **Authoritative Trusted Devices Re-Sync**:
  - Adopted host's authoritative `trusted_devices` list on the client during return synchronization, automatically propagating newly paired or revoked devices across all client vaults.
- **P2P Socket Interface Binding**:
  - Switched default sync listener binding from loopback (`127.0.0.1`) to all interfaces (`0.0.0.0:5322`), enabling cross-device mobile connections on local LANs.
- **Listener and Client Connection Hangs**:
  - Implemented non-blocking socket handling with bounded timeouts in `run_p2p_sync_listener` to prevent indefinite background thread lockups.
  - Replaced unbounded `TcpStream::connect` with `connect_timeout` (2s) in `run_p2p_sync_client`.
- **Sync Tag and State Desynchronization**:
  - Synchronized `refreshTags()` alongside `refreshEntries()` upon all P2P and WebDAV merge operations.
- **macOS Native Clipboard Argument Passing**:
  - Replaced broken `cat /dev/stdin` AppleScript pipe with `on run argv` script arguments in `crates/crypto/src/clipboard.rs`, resolving empty string returns during clipboard operations on macOS.
- **Mobile Safe Area & Notch Layout**:
  - Added `viewport-fit=cover`, `maximum-scale=1.0`, and `user-scalable=no` to the viewport meta tag in `index.html` for proper edge-to-edge rendering and zoom prevention.
  - Separated top safe-area padding (`env(safe-area-inset-top)`) from header height in the mobile detail view (`AppLayout.tsx`), preventing squashed headers on devices with Dynamic Island or notch.
- **Mobile Bottom Navigation & Scroll Margins**:
  - Dynamically sized the bottom navigation bar to `calc(4rem + env(safe-area-inset-bottom))` in `MobileBottomNav.tsx` to prevent icons and labels from compressing against the home bar.
  - Added bottom padding offsets to entry list and detail view scroll containers to prevent content from being occluded beneath the fixed navigation bar.
- **Mobile Drawer & Bottom Sheet Exit Animations**:
  - Moved conditional rendering inside `<AnimatePresence>` in `MobileDrawer.tsx` and `MobileBottomSheet.tsx`, enabling smooth Framer Motion exit transitions on close.
- **Mobile Onboarding Overflow**:
  - Replaced fixed `w-[420px]` container with `w-full max-w-[420px]` and `min-h-dvh overflow-y-auto` in `Onboarding.tsx`, eliminating horizontal clipping on narrow mobile viewports.
- **Mobile Localization Parity**:
  - Extracted hardcoded English strings in mobile views to translation keys (`mobile.back_to_vault`, `mobile.switch_vault`, `mobile.zero_knowledge_vault`) with Swedish and English definitions.

### Removed
- **Legacy Dual Sync Controls**:
  - Removed obsolete standalone "Sync Now via Wi-Fi" buttons and manual network listener toggles from Settings, unifying all local synchronization under Trusted Devices.

---

## [0.1.6] - 2026-09-15

### Added
- **Cryptographic Emergency Kit**:
  - Implemented 2-of-3 Shamir Secret Sharing recovery sheet generator with direct `.md` export.
  - Added encrypted audit logging for Emergency Kit generations, resets, and rekey invalidations.
- **Steam Guard TOTP Algorithm**:
  - Added dynamic HMAC-SHA1 alphanumeric 5-character token generation for Steam Guard accounts.
  - Added automatic detection for `steam://` URIs and `issuer=Steam`.
- **Storage Compaction & Trash Lifecycle**:
  - Added background storage compaction engine (`compact_vault`) and automatic 30-day trash purging (`purge_expired_trash`).
  - Added real-time storage metrics breaking down byte usage across active vs trashed items and attachments.
- **First-Run Onboarding Setup**:
  - Added setup wizard with Standard vs Closed System (air-gapped) privacy modes.
- **Security Dashboard Improvements**:
  - Clustered reciprocal reused password detections with direct per-entry remediation navigation.
  - Unified TOTP code view and 2FA recovery backup codes into a cohesive card.
- **Full Internationalization Parity (24 Languages)**:
  - Achieved complete key parity across all 24 supported locales (839 keys per language).

### Security
- **Smart Login ccTLD Base Domain Isolation**:
  - Enforced effective TLD+1 (`eTLD+1`) boundary matching with multi-part ccTLD parsing (`.co.uk`, `.com.au`), preventing credential leakage across public suffixes.
  - Restricted automated credential navigation to exact domain, same base domain, or explicit SSO pairings in `AUTH_DOMAINS`.
- **Smart Login CDP String Injection Prevention**:
  - Serialized all DOM-derived text targets using `serde_json::to_string` before JavaScript evaluation in Chrome DevTools Protocol.
- **Emergency Kit Keyed HMAC Fingerprinting**:
  - Replaced raw unkeyed hashes with keyed HMAC-SHA512 verification checksums derived from active session subkeys.
- **CSV Formula Injection Sanitization (CWE-1236)**:
  - Prepended escape single quotes to cell values starting with formula triggers (`=`, `+`, `-`, `@`, `\t`, `\r`) during CSV export.
- **Settings & Transit Memory Scrubbing on Vault Lock**:
  - Actively cleared WebDAV credentials, remote endpoints, and emergency audit logs from memory on lock.
  - Wrapped transient AEAD subkey vectors in `zeroize::Zeroizing` before transferring into `LockedBuffer`.
- **Hardware 2FA Argon2id Key Stretching**:
  - Enforced 256MB RAM Argon2id key derivation for Hardware 2FA envelope Key Encryption Keys.
- **Keyfile Factor Isolation**:
  - Removed plaintext keyfile path persistence from client browser storage.
- **Least Privilege Desktop Capabilities**:
  - Revoked clipboard-read permission and stripped development tools from production release capabilities.
- **Timing Attack Resistance**:
  - Enforced constant-time equality comparisons (`subtle::ConstantTimeEq`) across TOTP verification and local CLI daemon authentication.

### Fixed
- **Release-Mode Startup Crash Prevention**:
  - Replaced unhandled system tray window icon unwrap with safe pattern matching, preventing process aborts under `panic = "abort"`.
- **Tauri v2 Production CSP Dual-Scheme Parity**:
  - Added `https://ipc.localhost` to Content Security Policy `connect-src`, eliminating silent IPC invoke failures in Windows WebView2 release packages.
- **React Hook Lifecycle Ordering & Minified Errors**:
  - Relocated state hooks to top-level unconditional component scope, eliminating fatal `Minified React error #300`/`#310` crashes in production bundles.
- **Cross-Crate Hardware 2FA Test Gating**:
  - Removed `cfg!(debug_assertions)` wrappers from hardware 2FA and biometric test harnesses to ensure integration tests execute reliably under `cargo test --release`.
- **2FA Recovery Codes Mapping**:
  - Sanitized persistence pipeline so recovery backup codes are stored exclusively in dedicated fields and do not leak into custom fields.
- **KeePass XML Importer**:
  - Added support for protected in-memory values and XML entity decoding.

---

## [0.1.5] - 2026-09-14

### Fixed
- **Desktop SPA Route Resolution & Blank Screen Elimination**:
  - Migrated from `BrowserRouter` to `HashRouter` for desktop SPA protocol compatibility, preventing 404 blank screens when reloading pages in production release builds.
  - Implemented global `ErrorBoundary` UI component to capture unexpected render/layout exceptions, displaying recovery options and technical stack traces rather than leaving a blank screen.
  - Registered window runtime error and unhandled rejection listeners to ensure full diagnostic visibility in webview logs.
- **Framer Motion Reorder & State Stability**:
  - Refactored sidebar tag ordering in `Sidebar.tsx` to track stable primitive string IDs rather than volatile tag object instances, eliminating reference desynchronization and layout projection crashes during password saving and tag mutation.
  - Replaced `Reorder.Group` with standard list rendering when sorted by name or count, avoiding unnecessary animation frame projection overhead.
- **Tag ID Parity on Creation**:
  - Updated `addTag` optimistic update to record the real backend UUID returned from the database, preventing temporary client UUID divergence.
- **Atomic Save & Filesystem Watcher Race Guard**:
  - Added in-flight `.vdb.tmp` atomic save detection and a 200ms debounce in the background filesystem watcher thread to prevent false-positive connection lost events during vault file flushes.
- **Keyboard Text Input Caret Fix**:
  - Fixed an issue where outer container `select-none` styles prevented caret positioning and keyboard input in password fields while still permitting clipboard pasting.
- **Developer Tools & Native Inspection**:
  - Enabled Tauri `devtools` feature flag in release builds and added `Shift + Right Click` bypass to allow inspecting elements directly in production binaries.

---

## [0.1.4] - 2026-09-13

### Fixed
- **Smart Login Release Mode & Administrator Elevation Compatibility**:
  - Implemented automatic process de-elevation via Windows Explorer primary token duplication (`CreateProcessWithTokenW`) when Yntra Vault is executed with Administrator privileges, enabling Chromium-based browsers (Brave, Chrome, Edge) to launch with full sandbox security under the interactive desktop user session.
  - Added fallback compatibility mode with `--no-sandbox`, `--disable-gpu-sandbox`, and `--test-type` for non-shell/headless environments.
  - Corrected Chromium CLI extension flag from malformed `--disable-extensions-except=` to `--disable-extensions`.
  - Isolated standard streams (`Stdio::null()`) to prevent broken console handle inheritance under Windows GUI subsystem (`windows_subsystem = "windows"`).
  - Enforced browser executable parent directory as current working directory to prevent permission errors when launched from `Program Files`.
  - Enhanced stale lock cleanup by removing `SingletonCookie` and `SingletonSocket` alongside `SingletonLock` and `lockfile`.

---

## [0.1.3] - 2026-09-13

### Added
- **Complete Internationalization (24 Languages)**:
  - 100% localized translation coverage across all 24 supported locales: English (`en`), Swedish (`sv`), Danish (`da`), Norwegian (`no`), Finnish (`fi`), German (`de`), Dutch (`nl`), French (`fr`), Spanish (`es`), Italian (`it`), Portuguese (`pt`), Polish (`pl`), Czech (`cs`), Russian (`ru`), Ukrainian (`uk`), Turkish (`tr`), Greek (`el`), Hebrew (`he`), Arabic (`ar`), Hindi (`hi`), Japanese (`ja`), Korean (`ko`), Simplified Chinese (`zh-CN`), and Traditional Chinese (`zh-TW`).
  - Exactly 777 translated keys per language with 0 missing strings, validated by comprehensive i18n test suites.
- **Localized Interactive Tutorial & Onboarding**:
  - All walkthrough steps, tag names, password strength indicators, and credential placeholders in `VaultTutorial.tsx` are now fully translated.

### Changed
- **Package Identity**:
  - Corrected application package identifier in `package.json` from generic `my-app` template to official `yntra-vault`.

### Fixed
- **Translation Key Discrepancies**:
  - Fixed typo in `login.err_enter_password` key reference in `Login.tsx`.
- **Smart Login Browser Visibility**:
  - Removed `CREATE_NO_WINDOW` (`0x08000000`) flag during browser process spawn on Windows to ensure browser windows always open visibly in production release builds.
- **Process & Stale Lockfile Cleanup**:
  - Enhanced browser shutdown polling (3.5-second timeout) and automated purging of orphaned `SingletonLock` / lockfile artifacts in user data directories.

---

## [0.1.2] - 2026-09-13

### Fixed
- **Smart Login Release Process Spawning**:
  - Resolved headless flag and process creation flags causing Chromium instances to terminate silently in packaged release builds.
- **Browser Lockfile Handling**:
  - Added robust detection and removal of stale profile lockfiles before establishing Chrome DevTools Protocol (CDP) WebSocket sessions.

---

## [0.1.1] - 2026-09-13

### Added
- **Multi-Layer Hardware Key Wrapping (Hardware Envelopes)**:
  - Windows TPM 2.0 (`MS_PLATFORM_KEY_STORAGE_PROVIDER`) RSA key wrapping tagged with `b"YTPM"`.
  - Windows App-Bound DPAPI (`CryptProtectData`) with installation-bound BLAKE3 entropy tagged with `b"YDPB"`.
  - Backward compatibility detection for legacy untagged DPAPI blobs.
- **Passkey Authenticator**:
  - Native ECDSA over NIST P-256 (ES256) keypair generation and signing directly per entry.

### Changed
- **4-Crate Workspace Architecture**:
  - Decoupled monolithic core into clean Rust crates:
    - `crates/crypto` (`yntra-crypto`): Pure auditable cryptography, canary guard pages, and volatile zeroization.
    - `crates/core` (`yntra-vault-core`): Storage formats, database engine, manager CRUD, and services.
    - `crates/cli` (`yntra-cli`): High-speed CLI binary (`yntra.exe`) and session daemon.
    - `src-tauri` (`yntra-vault-app`): Desktop GUI wrapper.

---

## [0.1.0] - 2026-09-12

### Added
- **Core Cryptographic Storage Pipeline**:
  - `.vdb` binary vault format (v4) with single-pass XChaCha20-Poly1305 AEAD header binding.
  - Argon2id KDF (256 MB RAM, 4 iterations, 4 threads) + HKDF-SHA512 subkey derivation.
- **TOTP Authenticator**:
  - RFC 6238 compliant two-factor authentication generator (SHA-1, SHA-256, SHA-512).
- **Encrypted Search**:
  - Zero-disclosure trigram HMAC-SHA256 fuzzy search.
- **Autotype Engine**:
  - Windows UI Automation (UIA) input simulation with safety locks and window verification.
- **WebDAV Synchronization**:
  - Item-level 3-way merge with RFC 7232 ETag conditional requests and tombstone preservation.
- **CLI & Terminal TUI**:
  - High-performance command line client and Ratatui terminal user interface.
