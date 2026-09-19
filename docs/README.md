# Yntra Vault Documentation

Welcome to the Yntra Vault technical documentation. This directory houses architectural specifications, developer integration guides, reproducible build instructions, and security policies.

---

## Directory Overview

```text
docs/
├── README.md                      # This documentation index
├── README-public.md               # Public release template README
├── assets/                        # Brand and graphical assets
│   ├── LogoYP.jpg
│   └── WhiteLogoYP.png
├── security/                      # Formal cryptographic proofs & threat model
│   ├── cryptographic-proofs.md    # KDF hardness, AEAD proofs & memory hygiene
│   └── EMERGENCY_RECOVERY.md      # Shamir Secret Sharing emergency kit & recovery
├── architecture/                  # Architectural & data format specifications
│   ├── tech-spec.md               # System architecture & component design
│   ├── VDB_SPEC.md                # .vdb binary storage format specification
│   └── STORAGE_LIFECYCLE.md       # Trash retention, metrics & compaction lifecycle
└── development/                   # Developer guides & build pipelines
    ├── INTEGRATION-SDK.md         # IPC, autotype engine & browser integration
    └── REPRODUCIBLE_BUILDS.md     # Deterministic & reproducible build guide
```

---

## Core Documentation

### Security & Governance
* [**Changelog & Version History**](../CHANGELOG.md): Comprehensive version log documenting features, security updates, fixes, and architectural revisions.
* [**Security Policy & Vulnerability Disclosure**](../SECURITY.md): Vulnerability reporting instructions, response SLAs, scope, and GitHub Security Advisory protocols.
* [**Cryptographic Proofs & Security Model**](security/cryptographic-proofs.md): Formal Argon2id KDF hardness proofs, HKDF domain separation, XChaCha20-Poly1305 header AAD binding, zero-knowledge optical QR & PIN device pairing, adopt mode isolation, hardware guard page invariants, and k-anonymity proofs.
* [**Emergency Recovery & Shamir Kit**](security/EMERGENCY_RECOVERY.md): 2-of-3 Shamir Secret Sharing recovery kit generation, verification, and printable recovery sheets.

### Architecture & Format Specifications
* [**Technical Specification**](architecture/tech-spec.md): High-level system architecture, zero-knowledge P2P synchronization and optical QR/PIN pairing protocols, runtime dependencies, state management, WebDAV conflict resolution, animations, and theming.
* [**Storage Format Specification (.vdb)**](architecture/VDB_SPEC.md): Binary layout of `.vdb` files, version migration (v1 through v4), KDF parameter negotiation, and AEAD header authentication.
* [**Storage Compaction & Trash Lifecycle**](architecture/STORAGE_LIFECYCLE.md): 30-day trash tombstone expiration, storage metrics breakdown, and atomic database compaction.

### Developer & Integration Guides
* [**Integration SDK & Reference**](development/INTEGRATION-SDK.md): Browser integration protocols, autotype sequencing, IPC commands, and CLI session daemon interfaces.
* [**Deterministic & Reproducible Builds**](development/REPRODUCIBLE_BUILDS.md): Toolchain requirements, source remapping (`--remap-path-prefix`), and hermetic Docker verification.

### Repository & Assets
* [**Main Repository README**](../README.md): Primary project overview, environment setup, quick start, and security notices.
* [**Brand Assets**](assets/): High-resolution visual assets and logos.
