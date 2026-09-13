# Changelog

All notable changes to Yntra Vault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **Administrator Elevation Sandbox Compatibility**:
  - Detected elevated UAC execution via Windows `shell32::IsUserAnAdmin` and automatically appended `--no-sandbox` and `--disable-gpu-sandbox` flags when running with Administrator privileges to prevent Chromium sandbox initialization aborts.
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
