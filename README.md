# Yntra Vault

An offline-first, zero-knowledge desktop password manager engineered with Rust, Tauri 2, and React 19.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Security Policy](https://img.shields.io/badge/Security-Policy-red.svg)](SECURITY.md)
[![Format Spec](https://img.shields.io/badge/.vdb-Format_Spec-purple.svg)](docs/architecture/VDB_SPEC.md)
[![Languages: 24](https://img.shields.io/badge/Languages-24%20Supported-brightgreen.svg)](#internationalization-24-languages)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)

All credentials remain fully local on your device. Yntra Vault operates with zero cloud servers, zero telemetry, and zero mandatory third-party network connections. Full binary format specification is available in [VDB_SPEC.md](docs/architecture/VDB_SPEC.md).

---

> [!WARNING]
> **Pre-Audit Security Notice**: Yntra Vault is currently in active development. While built using multi-layer defense-in-depth cryptography and strict memory zeroization, the codebase **has not yet undergone an independent third-party security audit**. It is provided for community evaluation and testing. Please review [SECURITY.md](SECURITY.md) for vulnerability disclosure and [docs/security/cryptographic-proofs.md](docs/security/cryptographic-proofs.md) for formal proofs and security invariants.

---

## Key Security & Technical Highlights

### Defense-In-Depth Cryptography Pipeline

```mermaid
graph TD
    PWD["Master Password"] --> ARG["Argon2id KDF<br/>(256 MB RAM, 4 Iterations)"]
    ARG --> HKDF["HKDF-SHA512"]
    HKDF --> VK["Vault Key<br/>(XChaCha20-Poly1305 + Header AAD)"]
    HKDF --> EK["Entry Key<br/>(XChaCha20-Poly1305 / AES-256-GCM)"]
    HKDF --> HK["P2P Auth Key<br/>(HMAC-SHA512)"]
    HKDF --> SK["Search Key<br/>(Encrypted Index)"]
```

* **Single-Pass Authenticated Header**: Header metadata (magic, version, salt, KDF params) is bound as AAD into `XChaCha20-Poly1305`, authenticating header and payload before deserialization.
* **Zero-Knowledge P2P LAN Sync & Optical QR Device Pairing**: Direct local network synchronization over dedicated port 5324 using air-gapped optical QR pairing (`YQR2`) with out-of-band Short Authentication String (SAS) visual verification, dynamic session AAD binding, and zero-IPC credential isolation, alongside 6-digit PIN pairing with ephemeral Argon2id-derived transit subkeys, active UDP query-response discovery (`YQRY`), mutual HMAC challenge-response pre-authentication, strict adopt mode isolation, progressive two-step pairing wizard, and native desktop notifications.
* **Hardware Envelopes (TPM 2.0 & App-Bound DPAPI)**: Sensitive secrets and biometric session tokens are hardware-bound using Windows TPM 2.0 RSA encryption and BLAKE3 installation-bound DPAPI envelopes.
* **Passkey Support**: Native ES256 (ECDSA P-256) keypair generation and signing per entry.
* **Zeroize Memory Protection**: Critical keys and decrypted fields implement `zeroize::ZeroizeOnDrop` alongside guard-paged locked buffers (`PAGE_NOACCESS` / `mlock` + `MADV_DONTDUMP`).

---

## System Architecture

```mermaid
graph TD
    A["React 19 Frontend<br/><code>src/</code>"] -->|"IPC (Tauri Invoke)"| B["Tauri Shell<br/><code>src-tauri/</code>"]
    B -->|"Direct Core Calls"| C["Core Engine<br/><code>crates/core/ & crates/crypto/</code>"]
    B -->|"CLI Daemon & Tools"| CLI["CLI & Session Daemon<br/><code>crates/cli/</code>"]
    C -->|"Encrypted I/O"| D["Storage Payload<br/><code>.vdb File</code>"]
```

---

## Password & Authenticator Feature Suite

* **Item Types**: Login, Credit Card, Identity, Secure Note, SSH Key, API Key, Wi-Fi, Crypto Wallet.
* **TOTP Authenticator**: RFC 6238 compliant 2FA generator (SHA-1, SHA-256, SHA-512) with visual countdown.
* **Smart Login (CDP Engine)**: Automated browser login via Chrome DevTools Protocol with isolated browser sessions, automatic field detection, credential autofill, and elevated UAC token compatibility.
* **Password Generator**: CSPRNG character-set generator + Diceware passphrase engine.
* **Security Audit & Breach Check**: Local vault analyzer for weak/reused passwords + k-anonymity Have I Been Pwned lookup (transmitting only 5-character SHA-1 hash prefixes).
* **Autotype Engine**: OS credential input with field auto-classification (Windows UIA).
* **Encrypted Search**: Trigram-based fuzzy search executed locally without decrypting full entry payloads.

---

## Internationalization (24 Languages)

Yntra Vault is localized into 24 languages with 100% string coverage (824 translation keys per language, validated by automated test suites):

| Region | Supported Languages |
|:---|:---|
| **Nordic** | Swedish (`sv`), Danish (`da`), Norwegian (`no`), Finnish (`fi`) |
| **Western & Southern Europe** | English (`en`), German (`de`), Dutch (`nl`), French (`fr`), Spanish (`es`), Italian (`it`), Portuguese (`pt`) |
| **Central & Eastern Europe** | Polish (`pl`), Czech (`cs`), Russian (`ru`), Ukrainian (`uk`), Greek (`el`) |
| **Middle East & South Asia** | Turkish (`tr`), Hebrew (`he`), Arabic (`ar`), Hindi (`hi`) |
| **East Asia** | Japanese (`ja`), Korean (`ko`), Simplified Chinese (`zh-CN`), Traditional Chinese (`zh-TW`) |

---

## Feature Matrix & OS Compatibility

| Feature | Backend (`crates/`) | Frontend (`src`) | Supported OS | Status |
|:---|:---:|:---:|:---:|:---:|
| Vault Create / Unlock / Lock | `core::vault` | `CreateVaultModal.tsx` | Windows, macOS, Linux | ✅ Complete |
| Entry CRUD + Custom Fields | `core::vault` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| TOTP Authenticator (SHA1/256/512) | `core::totp` | `TOTPDisplay.tsx` | Windows, macOS, Linux | ✅ Complete |
| Smart Login Engine (CDP) | `core::smartlogin` | `SmartLoginModal.tsx` | Windows, macOS, Linux | ✅ Complete |
| Passkey Authenticator (ES256) | `crypto::passkey` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| Password Generator & Diceware | `core::generator` | `PasswordGenerator.tsx` | Windows, macOS, Linux | ✅ Complete |
| Security Audit & Breach Check | `core::breach` | `SecurityDashboard.tsx` | Windows, macOS, Linux | ✅ Complete |
| Encrypted Search (Trigram) | `core::vault::search` | `PasswordList.tsx` | Windows, macOS, Linux | ✅ Complete |
| Autotype Engine | `core::autotype` | `AutotypeButton.tsx` | Windows (UIA) | ✅ Windows |
| Master Password Re-keying | `core::vault` | `ChangeMasterPasswordModal.tsx` | Windows, macOS, Linux | ✅ Complete |
| Password History & Rollback | `core::vault::history` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| Hardware Envelopes (TPM 2.0 / DPAPI) | `crypto::tpm` | `Login.tsx` | Windows, macOS | ✅ Complete |
| Shamir Secret Sharing & Emergency Kit | `core::vault::emergency` | `SecurityTab.tsx` | Cross-Platform | ✅ Complete |
| WebDAV Cloud & Zero-Knowledge P2P Sync | `core::sync` | `DevicePairingWizard.tsx`, `SettingsPanel.tsx` | Cross-Platform | ✅ Complete |
| Optical QR-Code Device Pairing (YQR2) | `core::sync` | `QrScannerModal.tsx`, `DevicePairingWizard.tsx` | Cross-Platform | ✅ Complete |
| Command Line Interface (`yntra-cli`) | `cli/` | Terminal TUI (`yntra tui`) | Cross-Platform | ✅ Complete |
| 24 Locales & RTL Support | — | `src/i18n/` | Cross-Platform | ✅ Complete |

---

## Command Line Interface (`yntra` / `yntra-cli`)

Yntra Vault features an ultra-fast, SOTA command-line interface (`yntra` / `yntra-cli`) built with Rust `clap` (v4) and `ratatui` (v0.26).

### CLI Key Capabilities
- **Sub-5ms IPC Session Daemon (`yntra unlock`)**: Keeps unlocked vault state in zeroized memory over local Named Pipe / Unix Socket with `YNTRA_SESSION` token authentication and OS Credential Manager persistence.
- **Secret Injection Engine (`yntra run`)**: Resolves `yntra://<entry>/<field>` references directly in environment variables or `.env` template files (`yntra run --env-file .env.tpl -- <cmd>`) without writing secrets to disk.
- **Interactive Terminal UI (`yntra tui`)**: Ratatui + Crossterm TUI featuring fuzzy search (`/`), Vim navigation (`j`/`k`), live TOTP countdown gauge, modal creation dialogs (`a`), and defended clipboards (`c`/`t`).
- **Developer Ecosystem Protocols**:
  - `yntra git-credential setup`: One-click Git HTTPS credential helper setup.
  - `yntra ssh-agent`: OpenSSH Agent pipe server (`SSH_AUTH_SOCK`) for in-memory SSH authentication.
  - `yntra native-host install <chrome|firefox>`: Chrome/Firefox Native Messaging host installer.
- **Shell Auto-Completions**: Subcommand `yntra completions <zsh|bash|fish|powershell>`.

### Quick CLI Usage

```bash
# Build CLI binary
cargo build --release -p yntra-cli

# Unlock vault session daemon (sub-5ms command latency)
yntra unlock

# Launch interactive Terminal UI
yntra tui

# Inject secrets into process environment
yntra run --env-file .env.tpl -- npm start

# Configure Git HTTPS credential helper
yntra git-credential setup

# Generate Zsh completion script
yntra completions zsh > ~/.zsh/completion/_yntra
```

---

## Getting Started

### Prerequisites

#### 1. Common Dependencies
* **Rust**: `1.75.0` or higher ([Install Rust](https://www.rust-lang.org/tools/install))
* **Bun**: `1.0.0` or higher ([Install Bun](https://bun.sh)) or Node.js 18+

#### 2. System Build Tools (Required for Tauri)
* **Windows**: Visual Studio 2022 C++ Build Tools (`Desktop development with C++`) and Microsoft Edge WebView2 runtime.
* **Linux**: Install development packages:
  ```bash
  sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev libdbus-1-dev libglib2.0-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev
  ```
* **macOS**: Install Xcode Command Line Tools: `xcode-select --install`.

---

### Installation & Execution

```bash
# Clone the repository
git clone https://github.com/YntraAB/yntra-vault.git
cd yntra-vault

# Install JavaScript dependencies
bun install

# Launch in development mode
bun run tauri dev
```

### Building Production Installer

```bash
bun run tauri build
```

### Running Test Suite

```bash
# Workspace unit and integration tests
cargo test --workspace

# Frontend localization and unit tests
bun test

# Micro-benchmark suite
bun run bench
```

---

## Documentation & Format Specifications

* **Changelog & Releases**: [CHANGELOG.md](CHANGELOG.md)
* **Security Policy & Vulnerability Reporting**: [SECURITY.md](SECURITY.md)
* **Cryptographic Proofs & Security Model**: [docs/security/cryptographic-proofs.md](docs/security/cryptographic-proofs.md)
* **Storage Format Specification (.vdb)**: [VDB_SPEC.md](docs/architecture/VDB_SPEC.md)
* **Integration SDK & Architecture Reference**: [INTEGRATION-SDK.md](docs/development/INTEGRATION-SDK.md)
* **Reproducible Builds Specification & Verification**: [REPRODUCIBLE_BUILDS.md](docs/development/REPRODUCIBLE_BUILDS.md)
* **Documentation Index**: See [docs/README.md](docs/README.md)

---

## License

This project is open-source software licensed under the [MIT License](LICENSE).
