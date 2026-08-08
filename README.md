# Yntra Vault

An offline-first, zero-knowledge desktop password manager engineered with Rust, Tauri 2, and React 19.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Security Policy](https://img.shields.io/badge/Security-Policy-red.svg)](SECURITY.md)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)

All credentials remain fully local on your device. Yntra Vault operates with zero cloud servers, zero telemetry, and zero mandatory third-party network connections.

---

> [!WARNING]
> **Pre-Audit Security Notice**: Yntra Vault is currently in active development. While built using multi-layer defense-in-depth cryptography and strict memory zeroization, the codebase **has not yet undergone an independent third-party security audit**. It is provided for community evaluation and testing. Please review [SECURITY.md](SECURITY.md) for details on our threat model and security invariants.

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
* **Passkey Support**: Native ES256 (ECDSA P-256) keypair generation and signing per entry.
* **Zeroize Memory Protection**: Critical keys and decrypted fields implement `zeroize::ZeroizeOnDrop`. Memory pages are protected against process dump inspection via platform flags (`prctl` / `SetProcessMitigationPolicy`).

---

## System Architecture

```mermaid
graph TD
    A["React 19 Frontend<br/><code>src/</code>"] -->|"IPC (Tauri Invoke)"| B["Tauri Shell<br/><code>src-tauri/</code>"]
    B -->|"Direct Core Calls"| C["Core Engine<br/><code>yntra-vault-core (src-core/)</code>"]
    C -->|"Encrypted I/O"| D["Storage Payload<br/><code>.vdb File</code>"]
```

---

## Password & Authenticator Feature Suite

* **Item Types**: Login, Credit Card, Identity, Secure Note, SSH Key, API Key, Wi-Fi, Crypto Wallet.
* **TOTP Authenticator**: RFC 6238 compliant 2FA generator (SHA-1, SHA-256, SHA-512) with visual countdown.
* **Password Generator**: CSPRNG character-set generator + Diceware passphrase engine.
* **Security Audit & Breach Check**: Local vault analyzer for weak/reused passwords + k-anonymity Have I Been Pwned lookup (transmitting only 5-character SHA-1 hash prefixes).
* **Autotype Engine**: OS credential input with field auto-classification (Windows UIA).
* **Encrypted Search**: Trigram-based fuzzy search executed locally without decrypting full entry payloads.

---

## Feature Matrix & OS Compatibility

| Feature | Backend (`src-core`) | Frontend (`src`) | Supported OS | Status |
|:---|:---:|:---:|:---:|:---:|
| Vault Create / Unlock / Lock | `manager.rs` | `CreateVaultModal.tsx` | Windows, macOS, Linux | ✅ Complete |
| Entry CRUD + Custom Fields | `manager.rs` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| TOTP Authenticator (SHA1/256/512) | `totp/` | `TOTPDisplay.tsx` | Windows, macOS, Linux | ✅ Complete |
| Passkey Authenticator (ES256) | `crypto/passkey.rs` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| Password Generator & Diceware | `generator/` | `PasswordGenerator.tsx` | Windows, macOS, Linux | ✅ Complete |
| Security Audit & Breach Check | `breach/` | `SecurityDashboard.tsx` | Windows, macOS, Linux | ✅ Complete |
| Encrypted Search (Trigram) | `vault/search.rs` | `PasswordList.tsx` | Windows, macOS, Linux | ✅ Complete |
| Autotype Engine | `vault/autotype.rs` | `AutotypeButton.tsx` | Windows (UIA) | ✅ Windows |
| Master Password Re-keying | `manager.rs` | `ChangeMasterPasswordModal.tsx` | Windows, macOS, Linux | ✅ Complete |
| Password History & Rollback | `vault/history.rs` | `PasswordDetail.tsx` | Windows, macOS, Linux | ✅ Complete |
| Shamir Secret Sharing | `crypto/sharing.rs` | — | Cross-Platform | ⚙️ Core Only |
| WebDAV Cloud & P2P Vault Sync | `vault/sync.rs` | `SettingsPanel.tsx` | Cross-Platform | ✅ Complete |

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
# Core unit tests
cargo test --lib --manifest-path src-core/Cargo.toml

# Core micro-benchmark suite
bun run bench
```


---

## Documentation & Format Specifications

* **Security Policy & Vulnerability Reporting**: [SECURITY.md](SECURITY.md)
* **Reproducible Builds Specification & Verification**: [REPRODUCIBLE_BUILDS.md](REPRODUCIBLE_BUILDS.md)
* **Vault Storage Format (.vdb)**: [INTEGRATION-SDK.md](INTEGRATION-SDK.md)

---

## License

This project is open-source software licensed under the [MIT License](LICENSE).
