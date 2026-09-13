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
│   └── cryptographic-proofs.md    # KDF hardness, AEAD proofs & memory hygiene
├── architecture/                  # Architectural & data format specifications
│   ├── tech-spec.md               # System architecture & component design
│   └── VDB_SPEC.md                # .vdb binary storage format specification
└── development/                   # Developer guides & build pipelines
    ├── INTEGRATION-SDK.md         # IPC, autotype engine & browser integration
    └── REPRODUCIBLE_BUILDS.md     # Deterministic & reproducible build guide
```

---

## Core Documentation

### Security & Governance
* [**Security Policy & Vulnerability Disclosure**](../SECURITY.md): Vulnerability reporting instructions, response SLAs, scope, and GitHub Security Advisory protocols.
* [**Cryptographic Proofs & Security Model**](security/cryptographic-proofs.md): Formal Argon2id KDF hardness proofs, HKDF domain separation, XChaCha20-Poly1305 header AAD binding, hardware guard page invariants, and k-anonymity proofs.

### Architecture & Format Specifications
* [**Technical Specification**](architecture/tech-spec.md): High-level system architecture, frontend/backend runtime dependencies, state management, WebDAV conflict resolution, animations, and theming.
* [**Storage Format Specification (.vdb)**](architecture/VDB_SPEC.md): Binary layout of `.vdb` files, version migration (v1 through v4), KDF parameter negotiation, and AEAD header authentication.

### Developer & Integration Guides
* [**Integration SDK & Reference**](development/INTEGRATION-SDK.md): Browser integration protocols, autotype sequencing, IPC commands, and CLI session daemon interfaces.
* [**Deterministic & Reproducible Builds**](development/REPRODUCIBLE_BUILDS.md): Toolchain requirements, source remapping (`--remap-path-prefix`), and hermetic Docker verification.

### Repository & Assets
* [**Main Repository README**](../README.md): Primary project overview, environment setup, quick start, and security notices.
* [**Brand Assets**](assets/): High-resolution visual assets and logos.
