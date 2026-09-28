# Cryptographic Proofs & Security Model

This document describes cryptographic design arguments and intended security invariants in **Yntra Vault**. It is not an independent audit or a formal verification of the entire implementation.

**0.2.5 scope:** The later `YNS2` local envelope, random-secret recovery v2 and hardware-bound `YNTR` v5 are described in [the format specification](../architecture/VDB_SPEC.md) and [recovery specification](EMERGENCY_RECOVERY.md). Older password-derived diagrams and legacy recovery arguments below do not prove those implementations. Known-host-compromise, backup-revocation and updater-signature limits remain in [USB/recovery](USB-RECOVERY.md) and [updates](UPDATES.md).

---

## 1. System Overview & Security Posture

Yntra Vault is an **offline-first, zero-knowledge password manager** built in Rust and React/TypeScript. The security architecture enforces the following fundamental theorems:

1. **Key-based confidentiality**: Reading encrypted data requires the relevant key material. Recovery shares, an authorized linked device and an unlocked session are other access routes; possession of the original master password is not universally required. No remote key escrow or telemetry is introduced.
2. **Authenticated Envelope Binding**: Vault storage is tamper-evident. The cryptographic header (salt, KDF parameters, version) is cryptographically bound into the encryption envelope as Additional Authenticated Data (AAD), mathematically precluding header tampering and downgrade attacks.
3. **Memory protection**: Native key operations use guarded/page-locked buffers and zeroization where implemented. Display/copy operations and frontend/OS buffers can contain plaintext; this does not provide protection from a compromised host or guarantee every allocation is pinned.

---

## 2. Key Derivation Pipeline (KDF)

```
 Master Password (P) + CSPRNG Salt (32 bytes)
                    │
                    ▼
       ┌────────────────────────┐
       │       Argon2id         │  Memory: 256 MB (m = 262,144 KiB)
       │  (RFC 9106 Standard)   │  Iterations: 4 passes (t = 4)
       │                        │  Parallelism: 4 threads (p = 4)
       └───────────┬────────────┘
                   │
                   ▼  Master Key Material (64 bytes)
       ┌────────────────────────┐
       │      HKDF-SHA512       │  Extract: PRK = HMAC-SHA512(salt, MasterKey)
       │       (RFC 5869)       │  Expand:  OKM = HKDF-Expand(PRK, info, 32)
       └───────────┬────────────┘
                   │
    ┌──────────────┼──────────────┬──────────────┐
    ▼              ▼              ▼              ▼
VaultKey       EntryKey       SearchKey      P2pAuthKey
(32 bytes)     (32 bytes)     (32 bytes)     (32 bytes)
```

### 2.1 Argon2id Hardness & Resistance Properties
Yntra Vault adopts **Argon2id** (the hybrid mode of Argon2 combining Argon2i and Argon2d), recognized by the Password Hashing Competition (PHC) and specified in RFC 9106:

* **Memory Hardness**: The algorithm requires $256 \text{ MB}$ ($262{,}144 \text{ KiB}$) of RAM per derivation. On modern GPUs and ASICs, memory density and memory bandwidth form the primary economic bottlenecks. Dedicated cracking hardware cannot parallelize dictionary evaluations without allocating equivalent physical RAM per thread.
* **Side-Channel & TMTO Resistance**: The first half of the first iteration follows data-independent memory addressing (Argon2i rules), preventing cache-timing side-channel attacks. Subsequent passes utilize data-dependent addressing (Argon2d rules) to guarantee maximum resistance against Time-Memory Trade-Off (TMTO) attacks.
* **Salt Entropy**: Every vault initializes with a 32-byte (256-bit) salt generated via the operating system's Cryptographically Secure Pseudorandom Number Generator (CSPRNG, `rand::rngs::OsRng`). This guarantees global uniqueness and renders precomputed rainbow table attacks mathematically intractable ($2^{256}$ search space).

### 2.2 HKDF Subkey Separation Proof
The output of Argon2id yields 64 bytes of pseudo-random master key material ($MK$). To ensure cryptographic isolation between distinct subsystems, subkeys are derived using **HKDF-SHA512** (RFC 5869) with strictly distinct domain separation tags:

$$\text{SubKey}_i = \text{HKDF-Expand}\left(\text{HKDF-Extract}(\text{Salt}, MK), \text{info}_i, 32\right)$$

Where domain info strings are defined as:
* `yntra-vault-encryption-key-v1`: Vault-level outer payload envelope
* `yntra-vault-entry-encryption-key-v1`: Per-entry field-level encryption
* `yntra-vault-hmac-integrity-key-v1`: Legacy file HMAC and P2P shared-vault membership (not device identity)
* `yntra-vault-trigram-search-key-v1`: Blind trigram search indexing

**Security Invariant (Domain Separation)**:
Under the standard PRF assumption of HMAC-SHA512, compromise of any single subkey (e.g. `SearchKey` or an individual `EntryKey`) yields zero information regarding `VaultKey`, other subkeys, or the original master password.

---

## 3. Storage Envelope & Authenticated Encryption (AEAD)

### 3.1 Binary `.vdb` File Layout (v3 / v4 Format)

```
┌─────────────────────────────────────────────────────────────┐
│ Unencrypted FileHeader (Authenticated via AAD)             │
├─────────────────┬─────────────┬─────────────┬───────────────┤
│ Magic: "YNTR"   │ Version: u16│ Flags: u16  │ Salt: 32 B    │
│ (4 bytes)       │ (2 bytes)   │ (2 bytes)   │ (32 bytes)    │
├─────────────────┴─────────────┴─────────────┴───────────────┤
│ KDF Parameters (Argon2id m, t, p serialized length & data)  │
├─────────────────────────────────────────────────────────────┤
│ Payload Length: u64 (8 bytes, Little-Endian)                │
╞═════════════════════════════════════════════════════════════╡
│ Encrypted Payload (XChaCha20-Poly1305)                     │
├─────────────────────────────────────────────────────────────┤
│ Nonce: [u8; 24] (24 bytes random CSPRNG)                   │
├─────────────────────────────────────────────────────────────┤
│ Ciphertext (rmp-serde MessagePack serialized vault data)    │
├─────────────────────────────────────────────────────────────┤
│ Poly1305 Authentication Tag: [u8; 16]                       │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Authenticated Additional Data (AAD) Binding Proof
A primary vulnerability in naive password manager implementations is header malleability: an attacker modifying the version number, flags, or KDF parameters to force a downgrade or trigger memory exhaustion.

In Yntra Vault, the entire unencrypted header is bound as Additional Authenticated Data (`AAD`):

$$\text{AAD} = \text{Magic} \parallel \text{Version} \parallel \text{Flags} \parallel \text{Salt} \parallel \text{KDFParams} \parallel \text{PayloadLength}$$

$$\text{Ciphertext}, \text{Tag} = \text{XChaCha20-Poly1305-Encrypt}_{K_v}(N, \text{Payload}, \text{AAD})$$

**Integrity Verification Invariant**:
Before the payload is deserialized, the AEAD engine verifies $\text{Tag}$ over $(N, \text{Payload}, \text{AAD})$ in a single pass:

$$\text{Verify}(K_v, N, \text{Payload}, \text{AAD}, \text{Tag}) \stackrel{?}{=} \text{Valid}$$

* If an attacker modifies even a single bit of the unencrypted header (e.g. reducing Argon2 iterations or altering the salt), verification fails immediately.
* Deserialization of the MessagePack payload **never** executes if verification fails.
* The API returns a constant generic error (`VaultError::InvalidPassword` or `VaultError::IntegrityError`), preventing side-channel padding oracle or header leakage attacks.

### 3.3 Nonce Security (XChaCha20)
Standard ChaCha20 uses a 96-bit nonce, requiring strict state counters to avoid nonce reuse. Yntra Vault utilizes **XChaCha20-Poly1305** with an extended 192-bit (24-byte) random nonce generated per write via `OsRng`.

By the birthday paradox, the probability $p$ of a random nonce collision across $m$ encryption operations with an $n$-bit nonce is bounded by:

$$p \approx 1 - \exp\left(-\frac{m^2}{2^{n+1}}\right)$$

For $n = 192$, encrypting $m = 2^{32}$ ($4.29 \times 10^9$) vault revisions yields a collision probability $p < 2^{-129}$, which is cryptographically negligible.

---

## 4. In-Memory Security & Hardware Protections

Plaintext secrets in application memory are vulnerable to memory dumps, cold-boot attacks, swap-file leakage, and buffer overrun exploits. Yntra Vault enforces strict runtime defenses:

### 4.1 Hardware Canary Guard Pages (`LockedBuffer`)
Transient secrets (unwrapped passwords, raw cryptographic keys, passkey credentials) are managed through `LockedBuffer`:

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Leading Guard Page: PAGE_NOACCESS / PROT_NONE (4096 B)   │  <── Hardware Fault
├─────────────────────────────────────────────────────────────┤
│ 2. Protected Data Page: Read/Write (Page-Locked in RAM)     │  <── Physical RAM
├─────────────────────────────────────────────────────────────┤
│ 3. Trailing Guard Page: PAGE_NOACCESS / PROT_NONE (4096 B)  │  <── Hardware Fault
└─────────────────────────────────────────────────────────────┘
```

1. **Page Allocation**: Three contiguous virtual memory pages are reserved via `VirtualAlloc` (Windows) or `mmap` (Unix).
2. **Hardware Traps**: The leading and trailing guard pages are configured with `PAGE_NOACCESS` (Windows) or `PROT_NONE` (Unix). Any buffer underflow or overflow instantly triggers a hardware access violation / segmentation fault (`SIGSEGV`), terminating the process before memory inspection can occur.
3. **RAM Locking**: The center page is locked into physical RAM using `VirtualLock` (Windows) or `mlock` + `MADV_DONTDUMP` (Unix). This guarantees secrets are never paged to secondary storage (swap files, paging files, or hibernation images).
4. **Quota Adaptation**: If the system working set limit is exceeded, the process requests working set quota expansion via `SetProcessWorkingSetSize` (Windows) or `RLIMIT_MEMLOCK` (Unix).

### 4.2 In-Place Zero-Allocation Decryption
To eliminate dangling heap allocations:
* `ProtectedSecret::with_secret()` utilizes in-place decryption (`AeadInPlace::decrypt_in_place_detached`) directly inside a `LockedBuffer`.
* The plaintext secret exists exclusively inside the locked page for the execution duration of the closure.
* Upon closure completion, the buffer is immediately wiped using volatile memory writes (`std::ptr::write_volatile`) combined with a sequentially consistent compiler barrier (`std::sync::atomic::compiler_fence(Ordering::SeqCst)`), preventing compiler optimization dead-code elimination.

### 4.3 Scrambled Memory at Rest (`ScrambledString`)
Passwords displayed or held in memory during user interaction are stored as `ScrambledString`:
* Encrypted with an ephemeral master key (`static EPHEMERAL_KEY`) generated on application launch and held in page-locked memory.
* Individual strings are encrypted with ephemeral 24-byte nonces.
* RAM scanners reading the process address space observe only encrypted ciphertext blobs.

### 4.4 Process Core Dump Mitigation
At process initialization, runtime mitigations disable memory dumps:
* **Windows**: `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX)` suppresses crash dialogs and memory dump generation.
* **Unix**: `prctl(PR_SET_DUMPABLE, 0)` and `setrlimit(RLIMIT_CORE, 0)` prevent `gdb`, `ptrace`, and kernel crash dumps from capturing process memory.

---

## 5. Network Isolation & k-Anonymity Proof

### 5.1 Zero-Network Guarantee
Yntra Vault operates offline by default:
* No external telemetry endpoints, analytics, crash reporting, or cloud servers.
* Peer-to-peer sync operates over local encrypted channels with explicit user pairing.
* Cloud sync (WebDAV) functions strictly under user-configured and user-authenticated private remote servers.

### 5.2 Have I Been Pwned (HIBP) k-Anonymity Verification
When testing passwords against public breach databases, Yntra Vault implements strict **k-anonymity** via the Have I Been Pwned API:

1. **Client-Side Hashing**: The client computes the SHA-1 hash of the password:
   $$H = \text{SHA-1}(\text{Password}) \quad (\text{40 hexadecimal characters})$$
2. **Prefix Transmission**: The client extracts only the first 5 characters:
   $$H_{\text{prefix}} = H[0..5], \quad H_{\text{suffix}} = H[5..40]$$
3. **Range Query**: The client issues an HTTPS GET request to:
   $$\text{GET } \text{https://api.pwnedpasswords.com/range/}H_{\text{prefix}}$$
4. **Anonymity Set**: The API returns a list of approximately $500\text{--}1{,}000$ matching hash suffixes with their breach frequencies.
5. **Local Matching**: The client evaluates whether $H_{\text{suffix}}$ exists within the returned set entirely within local memory.

**Mathematical Privacy Guarantee**:
### 5.3 Peer-to-Peer Mutual Authentication Protocol
Version 0.2.5 uses `YNSYN003`. Shared vault keys establish vault membership; they do not identify a particular device. Each installation creates a separate Ed25519 signing key, wrapped with the platform key-wrapping mechanism, and pairing explicitly enrolls its public key in the local trusted-device list.

1. Both sides generate fresh 32-byte challenges and ephemeral P-256 ECDH key pairs. The length-prefixed transcript binds protocol version, both salt commitments, challenges, UUIDs, and ephemeral public keys.
2. The client proves shared-key possession with HMAC-SHA512 and signs the role-labelled transcript with its device key. The listener checks both proofs against its local enrollment before returning its own proofs.
3. The client verifies the listener's HMAC and pinned device signature before sending vault data. Empty trust lists, unknown or duplicate IDs, missing keys, and legacy peers fail closed and require explicit pairing; there is no shared-key-only compatibility fallback.
4. ECDH and HKDF-SHA256 derive a fresh session encryption key. XChaCha20-Poly1305 envelopes bind the transcript and transfer direction as AAD. Ordinary sync never replaces local device enrollment with a remote trusted-device list.

Removing a device blocks future authenticated sync access on the device where it was revoked, even if the removed device knows the shared vault keys or another device's UUID. Revocation does not erase prior copies or rotate shared inner vault keys: a former peer retaining those keys may decrypt a separately acquired compatible vault snapshot. These are design arguments and regression-tested invariants, not a formal proof or an independent protocol audit.

### 5.4 Keyed HMAC Emergency Kit Fingerprinting
Master password recovery sheets include an integrity checksum and audit log fingerprint. Rather than computing an unkeyed cryptographic digest of the raw master password (which would provide an offline brute-force verification oracle if the paper sheet is compromised), the fingerprint is computed as a keyed Pseudorandom Function (PRF):

$$\text{Fingerprint} = \text{Truncate}_8\left(\text{HMAC-SHA512}\left(\text{"yntra-vault-emergency-kit-audit-fingerprint-v1"}, K_{\text{hmac}}\right)\right)$$

Because $K_{\text{hmac}}$ is derived through Argon2id (256MB RAM, 4 passes) and HKDF-SHA512, an attacker possessing only the printed recovery sheet cannot evaluate candidate passwords against the fingerprint without allocating full Argon2id memory per attempt, and cannot precompute rainbow tables across vaults.

### 5.5 Exact-Origin & Explicit SSO Isolation (Smart Login)
Credential URLs are parsed with `url::Url`, reject userinfo, backslashes, and control characters, and require HTTPS for remote hosts. Credentials may be filled only at the exact saved origin (scheme, host, and effective port), or an explicitly listed pair of HTTPS hosts at port 443 in `AUTH_DOMAINS`. There is no base-domain, wildcard-subdomain, or public-suffix heuristic. Separate tenants such as `alice.pages.dev` and `bob.pages.dev` therefore remain separate, as do `example.com` and `login.example.com` unless explicitly enrolled as an SSO pair. HTTP is allowed only for explicit loopback diagnostics and still must match its exact origin.

The engine checks the current page immediately before each credential or OTP fill and submission. JavaScript fill operations also check the expected origin in the same evaluation; native Windows typing receives the freshly verified expected URL. Cancellation and the network policy are checked at these boundaries. Browser startup fails with a restart instruction if safe de-elevation fails; it never retries with `--no-sandbox`. A compromised permitted origin or privileged local process remains outside this origin check's protection.

### 5.6 CSV Formula Injection Sanitization (CWE-1236)
Decrypted vault exports to CSV format (`export_csv`) neutralize spreadsheet formula injection / DDE attacks. Any field whose initial character or trimmed character starts with `=`, `+`, `-`, `@`, `\t`, or `\r` is prepended with a single quote (`'`), instructing spreadsheet engines (Excel, LibreOffice Calc) to interpret the cell strictly as literal text.

### 5.7 Settings & Sensitive State Zeroization on Vault Lock
When `VaultManager::lock()` is called, `self.data.settings` is reset to `Default::default()`, actively purging WebDAV sync credentials, remote endpoints, and emergency kit audit logs from volatile memory alongside cryptographic keys, entries, tags, and search indices.

### 5.8 Keyfile 2FA Factor Isolation
Keyfiles represent a distinct "something you have" authentication factor. To prevent the physical location of keyfiles from being exposed to local non-privileged processes or web storage dumps, client applications are forbidden from storing keyfile filesystem paths in `localStorage` (`yntra-vault-keyfiles` or inside `yntra-vault-recent-vaults`).

### 5.9 Atomic Key-Wrap File Creation
Linux fallback key wrapping (`linux_get_or_create_wrap_key`) requires valid `XDG_CONFIG_HOME` or `HOME` directories, enforces `0o700` directory permissions, and creates the wrap key file atomically with mode `0o600` via `OpenOptionsExt::mode`, eliminating umask permission race conditions and rejecting insecure `/tmp` fallback paths.

### 5.10 PIN Pairing, Authenticated Enrollment & Session Binding
Device pairing permits instant, secure cross-device database synchronization using an ephemeral 6-digit numeric PIN without requiring USB file transfers:
1. **Pairing Secret Derivation**: Both devices derive a pairing key via BLAKE3 domain separation and Argon2id:
   $$\text{Salt}_{\text{pair}} = \text{BLAKE3}\left(\text{"yntra-pairing-salt-v1:"} \mathbin{\Vert} \text{Normalize}(\text{PIN})\right)$$
   $$K_{\text{master}} = \text{Argon2id}(\text{MasterPassword}, \text{Salt}_{\text{pair}}, m=256\text{MB}, t=4, p=4)$$
   $$\text{SubKeys} = \text{HKDF-SHA512}(K_{\text{master}})$$
2. **Ephemeral UDP Discovery Beacon Token**:
   $$\text{BeaconID} = \text{BLAKE3}_{\text{keyed}}\left(\text{BLAKE3}(K_{\text{hmac}}), \text{"yntra-pairing-beacon-v2"}\right)$$
   Because $\text{BeaconID}$ is keyed with the Argon2id-derived HMAC subkey, eavesdroppers on the local network cannot crack the 6-digit PIN offline without expending 256MB RAM per candidate attempt.
3. **Active UDP Query-Response Reflection & Amplification Resistance**:
   Clients emit 36-byte `YQRY` query payloads and hosts return 38-byte `YPAR` payloads only after matching the keyed beacon token. This limits payload amplification, but it is not a proof against reflection: observed tokens and spoofed source addresses still depend on network controls. In protocol v3, exchanged device metadata (including the signing public key) is MAC-authenticated with both fresh challenges and a role label. Both metadata records also enter the encrypted payload AAD, preventing replay, reflection, and enrollment-key substitution by an unauthenticated network peer. Reusing a PIN and password reuses pairing-derived keys; fresh challenges bind each transfer to its session.

### 5.11 Unauthenticated Client Isolation & Adopt Mode (`ClientPairingMode::AdoptIntoDir`)
When a client pairs while unauthenticated (e.g. from the `VaultSelect` start screen, where `state.vault = None`), the client is constrained to strict Adopt Mode:
- **No Existing Vault Read**: The client reads no existing local vault files or entries. The installation's separate signing identity is still used for pairing.
- **Zero Data Transmission**: The client transmits zero entries (`client_entries_count = 0`), preventing any leakage of unauthenticated or local credentials.
- **Filesystem Anti-Collision Invariant**: The adopted database is written to a dedicated non-colliding file (`<HostVaultName>.vdb`, `<HostVaultName> (1).vdb`), ensuring that existing local databases on disk are never modified or overwritten.

### 5.12 Multi-Interface Broadcast & Administrative Multicast Isolation
LAN discovery packets are simultaneously routed across:
1. Global broadcast (`255.255.255.255:5323`)
2. Administratively scoped local multicast (`239.255.53.23:5323` under RFC 2365 / `239.255.0.0/16`)
3. Subnet directed broadcasts (`x.y.z.255:5323`) for every network adapter detected by `get_local_lan_ips`.
Discovery is intended for the LAN. Actual routing and administrative boundaries depend on the host firewall, interfaces, and network configuration; discovery does not establish peer trust.

### 5.13 P2P Discovery Self-Echo Loop Isolation, Immediate Socket Release & Filesystem Neutralization
1. **In-Loop Self-Echo Isolation**: When devices actively listen for UDP discovery beacons on port 5323 while concurrently broadcasting their own presence, naive implementations risk terminating the discovery scan upon processing their own broadcast packet. `listen_discovery_beacon` matches received packet source IP addresses against the node's known local network interfaces and loopback *inside* the receive loop. Self-echo packets are dropped immediately and the loop continues, guaranteeing that discovery persists until external peers respond or the timeout expires.
2. **Bounded Cancellation & Session Deadline**: Accept loops and `CancellableStream` I/O poll cancellation and network policy, using short socket timeouts (200ms for stream I/O). Streams have an absolute session deadline, normally 90 seconds, so a peer cannot keep a connection alive indefinitely by trickling bytes. Cancellation releases sockets when the blocked operation next observes the flag; scheduling and non-I/O work can add latency.
3. **Windows DOS Reserved Device Name Sanitization**: Filenames derived during vault adoption (`ClientPairingMode::AdoptIntoDir`) are sanitized against reserved Windows DOS device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) in `sanitize_vault_filename`, preventing filesystem namespace collisions and denial-of-service conditions during initial pairing.

### 5.14 QR Pairing and Authenticated Device Enrollment (Transport `YQR3`)
The host generates a 256-bit random QR secret and a UUID session with a 90-second lifetime. The URI remains `yntrapair://v2` because its payload format is unchanged; the incompatible network transport is `YQR3` with `PAIR3_OK` acknowledgment. The QR includes connection hints and the secret, never a master password or vault payload. Anyone who obtains the QR secret during its lifetime has the corresponding pairing authority.

A client-first mutual HMAC exchange proves knowledge of the QR-derived keys before vault transfer. Both fresh challenges and role-labelled, authenticated metadata bind the enrolled device UUIDs and Ed25519 public keys. Encrypted transfer AAD binds that metadata transcript and the QR session ID. PIN pairing uses the same authenticated metadata rule. Old pairing transports must update; their unauthenticated metadata is not silently accepted.

The short authentication string is a visual confirmation aid. This is a local network transfer bootstrapped by an optical secret, not an air-gapped protocol. Network policy, cancellation, frame-size limits, and session deadlines apply. Received passwords are removed from the native result before serialization to the webview. A pending passwordless adoption holds native vault data and sync keys until the user selects a local wrapping password; cancellation and vault lock clear that pending state. It does not prove possession of the host's old master password.

---

## 6. Audit Verification & Compliance Checklist

| Security Property | Mechanism | File Location |
| ----------------- | --------- | ------------- |
| KDF Resistance | Argon2id ($m=256\text{MB}, t=4, p=4$) | `crates/crypto/src/kdf.rs` |
| Subkey Separation | HKDF-SHA512 with distinct info tags | `crates/crypto/src/kdf.rs` |
| Envelope AEAD | XChaCha20-Poly1305 + Header AAD | `crates/crypto/src/cipher.rs` |
| Field-Level AEAD | XChaCha20-Poly1305 / AES-256-GCM | `crates/crypto/src/cipher.rs` |
| RAM Locking & Canary | 3-Page `LockedBuffer` with `PAGE_NOACCESS` | `crates/crypto/src/locked_buffer.rs` |
| Volatile Zeroization | `ZeroizeOnDrop` + `write_volatile` | `crates/crypto/src/lib.rs` |
| Heap Scrambling | Ephemeral XChaCha20-Poly1305 | `crates/crypto/src/scrambled.rs` |
| Atomic File Writes | Temp file `write()` + atomic `rename()` | `crates/core/src/vault/manager.rs` |
| k-Anonymity Query | 5-char SHA-1 prefix over HTTPS | `crates/core/src/services/hibp.rs` |
| P2P Mutual Auth | Shared-key HMAC + pinned Ed25519 signatures + fresh P-256 ECDH | `crates/core/src/services/sync/mod.rs` and `identity.rs` |
| PIN Pairing | Password/PIN-derived keys + authenticated metadata and session AAD | `crates/core/src/services/sync/pairing.rs` |
| Timing-Safe Discovery | `subtle::ConstantTimeEq` across all UDP beacons | `crates/core/src/services/sync/mod.rs` & `pairing.rs` |
| Adopt Mode Isolation | No existing local vault reads and collision avoidance | `crates/core/src/services/sync/pairing.rs` |
| UDP Amplification Defense | Small bounded reply payload with keyed token gating | `crates/core/src/services/sync/pairing.rs` |
| Self-Echo Isolation | In-loop local IP dropping and continuation | `crates/core/src/services/sync/mod.rs` and `identity.rs` |
| Pairing Socket Release | Cancellable I/O, network policy, and absolute deadlines | `crates/core/src/services/sync/pairing.rs` & `src-tauri/src/commands/sync.rs` |
| DOS Device Sanitization | Windows reserved name neutralization on adopt | `crates/core/src/services/sync/pairing.rs` |
| Hardware 2FA KEK | Argon2id 256MB key stretching | `crates/crypto/src/hardware2fa.rs` |
| Constant-Time Verification | `subtle::ConstantTimeEq` comparisons | `crates/core/src/totp/mod.rs` & `crates/cli/src/ipc.rs` |
| Emergency Kit PRF Checksum | Keyed HMAC over session $K_{\text{hmac}}$ | `crates/core/src/vault/emergency.rs` |
| Credential Origin Isolation | Parsed exact HTTPS origins and explicit SSO pairs | `crates/core/src/smartlogin/discovery.rs` |
| CDP JS String Escaping | `serde_json::to_string` DOM serialization | `crates/core/src/smartlogin/engine.rs` |
| CSV Formula Defense | CWE-1236 quote-prefixing on export | `crates/core/src/vault/import_export.rs` |
| Settings Memory Scrubbing | Reset `self.data.settings` on `lock()` | `crates/core/src/vault/manager.rs` |
| Keyfile Factor Isolation | Strict exclusion from browser `localStorage` | `src/pages/Login.tsx` & `CreateVaultModal.tsx` |
| Atomic Wrap Key Creation | `OpenOptionsExt::mode(0o600)` on Unix | `crates/crypto/src/tpm.rs` |
| QR Pairing | QR secret + YQR3 HMAC and authenticated enrollment metadata | `crates/core/src/services/sync/pairing.rs` |
| Transit Dynamic AAD | Session ID, fresh challenges, roles, and device metadata | `crates/core/src/services/sync/pairing.rs` |
| Zero-IPC Secret Isolation | `PendingAdoptedVault` zeroized in native state | `src-tauri/src/commands/sync.rs` |
