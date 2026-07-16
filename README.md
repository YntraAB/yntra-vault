# Yntra Vault

An offline-first, zero-knowledge desktop password manager designed for high-security environments.

Yntra Vault operates fully locally and keeps your sensitive information private, utilizing standard cryptographic protocols to ensure security at every layer.

## Key Features

- **Multi-Layer Encryption Pipeline**:
  - **KDF**: Argon2id (256MB RAM, 4 passes) → HKDF-SHA512 → 3 subkeys (Vault, Entry, and HMAC Keys).
  - **Vault Level**: XChaCha20-Poly1305.
  - **Entry Level**: AES-256-GCM (each entry has its own unique, randomly generated key).
  - **Integrity**: HMAC-SHA512.
- **Zero-knowledge & Offline-first**: All data stays fully local on your device. There is no cloud storage, no trackers, and no unsolicited network requests.
- **TOTP Authenticator**: Built-in compliant 2FA generator supporting SHA-1, SHA-256, and SHA-512 with automatic countdown indicators.
- **Password History**: Roll back up to 10 previous passwords per entry.
- **Security Audit**: In-memory scan for duplicate, weak, old, or breached passwords.
- **Privacy-Friendly Breach Checks**: Real-time password check using the HIBP API with k-anonymity (transmits only the first 5 characters of the SHA-1 hash).

## Tech Stack

- **Backend**: Rust core (`yntra-vault-core`) + Tauri framework (`yntra-vault-app`)
- **Frontend**: React + TypeScript + Vite
- **Styling**: Tailwind CSS & Framer Motion for high-fidelity micro-interactions

## Development Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js / Bun](https://bun.sh)

### Installation & Run

1. Clone this repository:
   ```bash
   git clone <repository-url>
   cd yntra-vault
   ```

2. Install dependencies:
   ```bash
   bun install
   ```

3. Launch the desktop application in development mode:
   ```bash
   bun run tauri dev
   ```

4. Build production installers:
   ```bash
   bun run tauri build
   ```

## License

This project is licensed under the MIT License.
