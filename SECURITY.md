# Security Policy & Vulnerability Disclosure

## Security Status

> **Notice**: Yntra Vault is currently in active development. While designed with defense-in-depth cryptographic primitives, the codebase **has not yet undergone an independent third-party security audit**. It is provided for evaluation, testing, and community review.

---

## Threat Model & Security Invariants

Yntra Vault operates under an **offline-first, zero-knowledge** security posture.

### 1. Cryptographic Isolation
* **Key Derivation**: Master Password → Argon2id (256MB RAM, 4 iterations, 4 parallelism) → HKDF-SHA512 → Derived SubKeys (`Vault Key`, `Entry Key`, `Search Key`, `HMAC / P2P Auth Key`).
* **Vault Encryption**: XChaCha20-Poly1305 with unencrypted `FileHeader` bound as Additional Authenticated Data (`AAD`).
* **Entry Field Encryption**: XChaCha20-Poly1305 / AES-256-GCM for sensitive fields within individual entries.
* **Integrity Guarantee**: Single-pass Poly1305 AEAD tag authentication over header AAD + payload (rejecting tampered headers or payloads before payload deserialization; legacy v1/v2 files verify outer HMAC-SHA512 first).

### 2. Memory Hygiene
* All key buffers (`SubKeys`), temporary secrets (`LockedBuffer`), and scrambled UI memory (`ScrambledString`) implement `ZeroizeOnDrop` via the `zeroize` crate.
* Core dump generation is disabled at runtime using platform mitigation APIs (`prctl(PR_SET_DUMPABLE, 0)` & `setrlimit(RLIMIT_CORE, 0)` on Unix / `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX)` on Windows).
* Memory pages holding raw keys use OS page locking (`VirtualLock` / `mlock`) where supported.

### 3. Network & Telemetry Policy
* **Zero Network Calls**: No telemetry, analytics, background checks, or cloud sync by default.
* **k-Anonymity HIBP Integration**: Breach checking queries Have I Been Pwned by transmitting **only** the first 5 characters of the SHA-1 hash of a password over HTTPS. Raw passwords or full hashes are never transmitted.

---

## Reporting a Vulnerability

If you discover a security vulnerability or cryptographic flaw in Yntra Vault, please report it responsibly rather than opening a public issue.

* **Private Contact**: Contact the project owners/maintainers privately (e.g. via private messaging or Keybase/socials linked on owner profiles) rather than opening public GitHub issues.
* **Response Timeline**:
  * **Acknowledgement**: Within 48 hours.
  * **Assessment & Fix Plan**: Within 7 business days.
  * **Public Release**: Coordinated advisory published upon release of a patched version.
