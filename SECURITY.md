# Security Policy & Vulnerability Disclosure

Yntra Vault is an offline-first, high-security password manager engineered with defense-in-depth cryptographic primitives and strict memory safety guarantees. We take the security of our users and their data seriously.

---

## Supported Versions

Security fixes and advisories are prioritized for active development releases:

| Version | Supported          | Status |
| ------- | ------------------ | ------ |
| `0.1.x` | :white_check_mark: | Active Development / Evaluation |
| `< 0.1` | :x:                | Unsupported / Deprecated |

> **Pre-Audit Notice**: Yntra Vault is currently in active development. While built with defense-in-depth cryptographic primitives and hardware-assisted memory locking, the codebase **has not yet undergone an independent third-party security audit**. It is provided for evaluation, testing, and community security review.

---

## Reporting a Vulnerability

If you discover a security vulnerability, cryptographic weakness, or memory safety flaw in Yntra Vault, please report it **privately and responsibly**. Do not open public GitHub issues or discussions for unpatched vulnerabilities.

### Reporting Channels

1. **GitHub Security Advisories (Preferred)**:
   - Navigate to the [Security Advisories](https://github.com/YntraAB/yntra-vault/security/advisories/new) tab on GitHub and click **"Report a vulnerability"**.
   - This provides an encrypted, private channel directly to project maintainers.

2. **Direct Contact**:
   - For issues requiring off-platform communication or PGP-encrypted disclosure, contact project maintainers directly via private channels linked on owner GitHub profiles (`@YntraAB`).

### What to Include

Please provide detailed information to help us triage and verify the issue promptly:
- Description of the vulnerability and its potential impact.
- Step-by-step reproduction instructions or a minimal Proof-of-Concept (PoC).
- Affected components (e.g. `src-core/src/crypto/`, `LockedBuffer`, Tauri IPC, frontend).
- Platform and operating system details (Windows, Linux, macOS) and target architecture.
- Any suggested mitigations or patches (optional).

### Response SLA & Disclosure Timeline

| Milestone | Target Window | Description |
| --------- | ------------- | ----------- |
| **Initial Acknowledgment** | ≤ 48 hours | Confirmation that the report was received and initial triage has started. |
| **Triage & Assessment** | ≤ 7 business days | Reproduction, severity rating (CVSS), and proposed remediation plan. |
| **Patch & Verification** | ≤ 30 days | Implementation of fix, regression testing, and security verification. |
| **Coordinated Disclosure** | Post-release | Public release of patched binary and published GitHub Security Advisory with researcher credit. |

---

## Scope & Threat Model Boundaries

### In-Scope
- Cryptographic design or implementation flaws (Argon2id, HKDF-SHA512, XChaCha20-Poly1305, AES-256-GCM).
- Storage integrity flaws, ciphertext substitution, or header AAD bypass in `.vdb` files.
- Memory protection bypasses (leakage of unencrypted keys or secrets past `LockedBuffer` / `ScrambledString`).
- IPC command injection or privilege escalation across the Tauri backend interface.
- Data leaks violating the zero-knowledge offline policy (e.g. unexpected network connections).
- Breach monitor data leaks (any transmission beyond the 5-character SHA-1 prefix for k-anonymity queries).

### Out-of-Scope
- Attacks requiring existing root / administrator / kernel-level malware on the host machine.
- Physical attacks on an unlocked workstation where the attacker has uninterrupted physical access.
- Denial-of-Service attacks requiring local process termination via external OS tools (`taskkill`, `kill -9`).
- Social engineering attacks targeting users directly.

---

## Technical Security & Cryptographic Proofs

For complete technical specifications, formal proofs, and memory protection invariants, refer to our comprehensive security documentation:

* [**Technical Cryptographic Proofs & Threat Model**](docs/security/cryptographic-proofs.md)
* [**Storage Format Specification (.vdb)**](docs/architecture/VDB_SPEC.md)
* [**System Architecture & Security Controls**](docs/architecture/tech-spec.md)
