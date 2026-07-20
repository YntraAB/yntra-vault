# Yntra Vault

An offline-first, zero-knowledge desktop password manager built with Rust, Tauri, and React.

All data stays fully local — no cloud, no telemetry, no third-party dependencies at runtime.

---

## Features

### Security
- **Multi-layer encryption**: Argon2id → HKDF-SHA512 → XChaCha20-Poly1305 (vault) + AES-256-GCM (per-entry)
- **HMAC-SHA512** integrity verification before decryption
- **Zero-knowledge**: master password never leaves your device
- **Memory safety**: all sensitive data zeroized on drop/lock (via `zeroize` crate)
- **Passkey support**: ES256 (ECDSA P-256) keypair generation per entry

### Password Management
- **Entry types**: Login, Credit Card, Identity, Secure Note, SSH Key, API Key, Wi-Fi, Crypto Wallet
- **Custom fields**: arbitrary key-value pairs per entry
- **Password history**: rollback to previous passwords
- **Tags & favorites**: organize and pin entries
- **Soft delete**: 30-day trash with automatic cleanup

### Tools
- **TOTP Authenticator**: built-in 2FA (SHA-1, SHA-256, SHA-512) with countdown timer
- **Password Generator**: random (configurable charset) + Diceware (word-based)
- **Security Audit**: scan for weak, reused, old, and breached passwords
- **Breach Check**: HIBP API with k-anonymity (only 5-char SHA-1 prefix sent)
- **Autotype**: OS-level credential input with field classification and window lock
- **Browser Integration**: Chrome/Firefox/Edge native messaging host
- **Encrypted Search**: trigram-based fuzzy search over HMAC-hashed index

### File Format
- **`.vdb` vault file**: versioned binary format with automatic migration
- **v2 payload**: MessagePack (self-describing) — future field additions never break old vaults
- **Export**: copy vault file to custom location via file dialog

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Core Engine | Rust (`yntra-vault-core`) |
| Desktop Shell | Tauri 2 (`yntra-vault-app`) |
| Frontend | React 19 + TypeScript + Vite |
| Styling | Tailwind CSS + Framer Motion |
| Fonts | Geist Sans + Geist Mono |

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Bun](https://bun.sh) (or Node.js)

### Install & Run

```bash
git clone <repository-url>
cd yntra-vault
bun install
bun run tauri dev
```

### Build Production Installer

```bash
bun run tauri build
```

### Run Tests

```bash
cargo test --lib --manifest-path src-core/Cargo.toml
```

---

## License

MIT
