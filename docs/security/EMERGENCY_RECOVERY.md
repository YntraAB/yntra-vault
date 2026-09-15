# Cryptographic Emergency Recovery Kit Specification

> Technical specification of Yntra Vault's 2-of-3 Shamir Secret Sharing recovery kit, threshold algebra, self-verification, and reconstruction procedures.

---

## 1. Overview

Yntra Vault operates on an offline-first, zero-knowledge architecture. There are no central servers, recovery hotlines, or backdoor password reset mechanisms. To protect users against catastrophic loss of their master password without introducing third-party escrow risk, Yntra Vault provides a **Cryptographic Emergency Kit**.

The Emergency Kit generates three physical recovery shares using Shamir's Secret Sharing scheme ($k = 2, n = 3$).
- **Threshold**: Any two shares can reconstruct the original master password.
- **Information-Theoretic Secrecy**: Any single share reveals zero mathematical information about the master password ($H(S \mid S_i) = H(S)$).
- **Physical Distribution**: Users can store Share A in a home fireproof safe, Share B in a bank deposit box, and Share C with a trusted family member or legal representative. Compromising any single location yields zero access.

---

## 2. Mathematical Foundation

### Galois Field $\text{GF}(2^8)$ Arithmetic
All operations are evaluated over the finite field $\text{GF}(2^8)$ defined by the AES Rijndael irreducible polynomial:

$$P(x) = x^8 + x^4 + x^3 + x + 1 \quad (0\text{x}11\text{B})$$

For each byte $s$ of the master password, an ephemeral linear polynomial is constructed using hardware cryptographically secure random bytes ($a_1 \leftarrow \text{OsRng}$):

$$f(x) = s + a_1 x \pmod{P(x)}$$

### Share Evaluation Points
The shares are evaluated at non-zero field points $x \in \{1, 2, 3\}$:
- **Share 1 ($x=1$)**: $y_1 = f(1) = s \oplus a_1$
- **Share 2 ($x=2$)**: $y_2 = f(2) = s \oplus (a_1 \bullet 2)$
- **Share 3 ($x=3$)**: $y_3 = f(3) = s \oplus (a_1 \bullet 3)$

### Lagrange Interpolation
Given any pair of shares $(x_A, y_A)$ and $(x_B, y_B)$ with $x_A \neq x_B$, the secret byte $s = f(0)$ is recovered in constant time via:

$$s = y_A \frac{0 - x_B}{x_A - x_B} + y_B \frac{0 - x_A}{x_B - x_A} = y_A \frac{x_B}{x_A \oplus x_B} \oplus y_B \frac{x_A}{x_A \oplus x_B}$$

---

## 3. Self-Verification & Checksums

To guarantee that a generated recovery sheet contains valid, reconstitutable shares prior to being exported or printed:
1. **Automated Round-Trip Validation**:
   The generator immediately pairs Share 1 and Share 2, reconstructs the secret, and compares it against the input password.
2. **Share Fingerprinting**:
   Each share includes a truncated SHA-256 fingerprint (8 hexadecimal characters) displayed alongside the share data. This allows users to confirm share integrity and prevent transcription errors.
3. **Memory Safety**:
   Transient secret buffers are held in hardware page-locked memory (`LockedBuffer`) and immediately zeroed upon completion (`zeroize`).
4. **Pre-Generation Cryptographic Verification**:
   Before generating recovery shares or writing audit logs, `VaultManager::generate_emergency_kit` validates the candidate master password against active session keys via Argon2id, HKDF, and constant-time comparison (`subtle::ConstantTimeEq`). Invalid passwords immediately return `VaultError::InvalidPassword` without modifying vault state.
5. **Encrypted In-Vault Audit Trail**:
   All kit generations, manual resets, and rekey invalidations are tracked with SHA-256 fingerprints and UTC timestamps inside `EmergencyKitAudit` within the encrypted `.vdb` settings payload.

---

## 4. Emergency Kit Layout

The generator outputs a structured Markdown recovery sheet containing:
1. **Vault Identity**: Vault title, unique vault UUID, and creation timestamp.
2. **Security Advisories**: Clear instructions on split storage and physical handling.
3. **Recovery Shares**:
   - Share 1 (Evaluation point $x=1$ with fingerprint)
   - Share 2 (Evaluation point $x=2$ with fingerprint)
   - Share 3 (Evaluation point $x=3$ with fingerprint)
4. **Reconstruction Steps**: Step-by-step CLI commands (`yntra recover`) and GUI input instructions.

---

## 5. API & IPC Surface

- **Rust Core**: `crates/core/src/vault/emergency.rs` & `crates/core/src/vault/manager.rs`
  - `VaultManager::verify_master_password(&self, candidate: &str) -> crate::Result<bool>`
  - `VaultManager::generate_emergency_kit(&mut self, master_password: &str) -> crate::Result<EmergencyKit>`
  - `VaultManager::get_emergency_kit_audit(&self) -> Option<EmergencyKitAudit>`
  - `VaultManager::reset_emergency_kit_audit(&mut self) -> crate::Result<()>`
- **Tauri IPC Commands**:
  - `generate_emergency_kit`: `{ masterPassword: string }` -> `EmergencyKit`
  - `get_emergency_kit_audit`: `()` -> `Option<EmergencyKitAudit>`
  - `reset_emergency_kit_audit`: `()` -> `()`
- **Frontend SDK**:
  - `backend.generateEmergencyKit(masterPassword)`
  - `backend.getEmergencyKitAudit()`
  - `backend.resetEmergencyKitAudit()`
