# Cryptographic Proofs & Security Model

This document establishes the formal cryptographic foundations, threat model, and defense-in-depth security invariants implemented in **Yntra Vault**.

---

## 1. System Overview & Security Posture

Yntra Vault is an **offline-first, zero-knowledge password manager** built in Rust and React/TypeScript. The security architecture enforces the following fundamental theorems:

1. **Zero Knowledge**: Data at rest and data in transit remain unreadable without the user's master password. The software maintains zero external telemetry, zero tracking, and zero remote key escrow.
2. **Authenticated Envelope Binding**: Vault storage is tamper-evident. The cryptographic header (salt, KDF parameters, version) is cryptographically bound into the encryption envelope as Additional Authenticated Data (AAD), mathematically precluding header tampering and downgrade attacks.
3. **Defense-in-Depth Memory Isolation**: Ephemeral keys and decrypted secrets never reside as plaintext strings in unpinned, heap-allocated memory. Sensitive operations run within hardware-guarded, page-locked memory buffers (`LockedBuffer`).

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
* `yntra-vault-hmac-integrity-key-v1`: Legacy HMAC and P2P session authentication
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
To prevent unauthenticated password oracles during local network synchronization, the P2P sync protocol implements strict client-first verification:
1. Listener generates a 32-byte CSPRNG challenge (`server_challenge`) and transmits it to the peer.
2. Client computes `client_sig = HMAC-SHA512(server_challenge, hmac_key)` and transmits `(client_challenge, client_sig)`.
3. Listener verifies `client_sig` using constant-time verification *before* computing or transmitting any response. Connections failing verification are terminated with `UNAUTHOR`.
4. Only upon successful client verification does the listener compute `server_sig = HMAC-SHA512(client_challenge, hmac_key)` and return authentication confirmation.

### 5.4 Keyed HMAC Emergency Kit Fingerprinting
Master password recovery sheets include an integrity checksum and audit log fingerprint. Rather than computing an unkeyed cryptographic digest of the raw master password (which would provide an offline brute-force verification oracle if the paper sheet is compromised), the fingerprint is computed as a keyed Pseudorandom Function (PRF):

$$\text{Fingerprint} = \text{Truncate}_8\left(\text{HMAC-SHA512}\left(\text{"yntra-vault-emergency-kit-audit-fingerprint-v1"}, K_{\text{hmac}}\right)\right)$$

Because $K_{\text{hmac}}$ is derived through Argon2id (256MB RAM, 4 passes) and HKDF-SHA512, an attacker possessing only the printed recovery sheet cannot evaluate candidate passwords against the fingerprint without allocating full Argon2id memory per attempt, and cannot precompute rainbow tables across vaults.

### 5.5 Multi-Part ccTLD & SSO Boundary Isolation (Smart Login)
Automated credential autofill enforces strict effective top-level domain (`eTLD+1`) boundary isolation. Naive domain extraction splitting on the final two components collapses distinct organizations sharing a multi-part country code (e.g. `victim.co.uk` and `attacker.co.uk` collapsing to `co.uk`), allowing cross-tenant credential injection. Yntra Vault's `base_domain` incorporates an explicit list of multi-part ccTLD public suffixes and ccTLD heuristics to guarantee that `eTLD+1` matches exactly before credentials can be populated. Furthermore, SSO auth domain pairings (`AUTH_DOMAINS`) enforce strict boundary and dot-prefix matching.

### 5.6 CSV Formula Injection Sanitization (CWE-1236)
Decrypted vault exports to CSV format (`export_csv`) neutralize spreadsheet formula injection / DDE attacks. Any field whose initial character or trimmed character starts with `=`, `+`, `-`, `@`, `\t`, or `\r` is prepended with a single quote (`'`), instructing spreadsheet engines (Excel, LibreOffice Calc) to interpret the cell strictly as literal text.

### 5.7 Settings & Sensitive State Zeroization on Vault Lock
When `VaultManager::lock()` is called, `self.data.settings` is reset to `Default::default()`, actively purging WebDAV sync credentials, remote endpoints, and emergency kit audit logs from volatile memory alongside cryptographic keys, entries, tags, and search indices.

### 5.8 Keyfile 2FA Factor Isolation
Keyfiles represent a distinct "something you have" authentication factor. To prevent the physical location of keyfiles from being exposed to local non-privileged processes or web storage dumps, client applications are forbidden from storing keyfile filesystem paths in `localStorage` (`yntra-vault-keyfiles` or inside `yntra-vault-recent-vaults`).

### 5.9 Atomic Key-Wrap File Creation
Linux fallback key wrapping (`linux_get_or_create_wrap_key`) requires valid `XDG_CONFIG_HOME` or `HOME` directories, enforces `0o700` directory permissions, and creates the wrap key file atomically with mode `0o600` via `OpenOptionsExt::mode`, eliminating umask permission race conditions and rejecting insecure `/tmp` fallback paths.

### 5.10 Zero-Knowledge Device Pairing Protocol & Ephemeral Transit Keys
Device pairing permits instant, secure cross-device database synchronization using an ephemeral 6-digit numeric PIN without requiring USB file transfers:
1. **Pairing Secret Derivation**: Both devices derive an ephemeral pairing key via BLAKE3 domain separation and Argon2id:
   $$\text{Salt}_{\text{pair}} = \text{BLAKE3}\left(\text{"yntra-pairing-salt-v1:"} \mathbin{\Vert} \text{Normalize}(\text{PIN})\right)$$
   $$K_{\text{master}} = \text{Argon2id}(\text{MasterPassword}, \text{Salt}_{\text{pair}}, m=256\text{MB}, t=4, p=4)$$
   $$\text{SubKeys} = \text{HKDF-SHA512}(K_{\text{master}})$$
2. **Ephemeral UDP Discovery Beacon Token**:
   $$\text{BeaconID} = \text{BLAKE3}_{\text{keyed}}\left(\text{BLAKE3}(K_{\text{hmac}}), \text{"yntra-pairing-beacon-v2"}\right)$$
   Because $\text{BeaconID}$ is keyed with the Argon2id-derived HMAC subkey, eavesdroppers on the local network cannot crack the 6-digit PIN offline without expending 256MB RAM per candidate attempt.
3. **Active UDP Query-Response Reflection & Amplification Resistance**:
   Clients emit query pulses $\text{Packet}_{\text{query}} = [\text{"YQRY"} \mathbin{\Vert} \text{BeaconID}]$ (36 bytes). Hosts verify the token in constant time (`ConstantTimeEq`) and respond with $\text{Packet}_{\text{reply}} = [\text{"YPAR"} \mathbin{\Vert} \text{BeaconID} \mathbin{\Vert} \text{Port}_{\text{tcp}}]$ (38 bytes). The response-to-request byte ratio is $38/36 \approx 1.05$, providing mathematical proof of zero traffic amplification. Unauthenticated reflection is impossible without knowledge of the Argon2id-derived $\text{BeaconID}$.

### 5.11 Unauthenticated Client Isolation & Adopt Mode (`ClientPairingMode::AdoptIntoDir`)
When a client pairs while unauthenticated (e.g. from the `VaultSelect` start screen, where `state.vault = None`), the client is constrained to strict Adopt Mode:
- **Zero Local Read Invariant**: The client opens zero files from the filesystem and decrypts zero local storage blocks.
- **Zero Data Transmission**: The client transmits zero entries (`client_entries_count = 0`), preventing any leakage of unauthenticated or local credentials.
- **Filesystem Anti-Collision Invariant**: The adopted database is written to a dedicated non-colliding file (`<HostVaultName>.vdb`, `<HostVaultName> (1).vdb`), ensuring that existing local databases on disk are never modified or overwritten.

### 5.12 Multi-Interface Broadcast & Administrative Multicast Isolation
LAN discovery packets are simultaneously routed across:
1. Global broadcast (`255.255.255.255:5323`)
2. Administratively scoped local multicast (`239.255.53.23:5323` under RFC 2365 / `239.255.0.0/16`)
3. Subnet directed broadcasts (`x.y.z.255:5323`) for every network adapter detected by `get_local_lan_ips`.
All discovery traffic is strictly restricted to local administrative boundaries and will not route beyond the local autonomous system or private network gateway.

### 5.13 P2P Discovery Self-Echo Loop Isolation, Immediate Socket Release & Filesystem Neutralization
1. **In-Loop Self-Echo Isolation**: When devices actively listen for UDP discovery beacons on port 5323 while concurrently broadcasting their own presence, naive implementations risk terminating the discovery scan upon processing their own broadcast packet. `listen_discovery_beacon` matches received packet source IP addresses against the node's known local network interfaces and loopback *inside* the receive loop. Self-echo packets are dropped immediately and the loop continues, guaranteeing that discovery persists until external peers respond or the timeout expires.
2. **Atomic Pairing Socket Release & Instant Cancellation**: Host pairing listeners on port 5324 poll an atomic cancellation flag (`pairing_cancel: Arc<AtomicBool>`) every 40ms via the Tauri IPC command `cancel_pairing_host`. Listening sockets are released immediately upon user cancellation or modal dismissal, preventing port lockups on TCP 5324 and UDP 5323 and eliminating dangling connection acceptances after session termination.
3. **Windows DOS Reserved Device Name Sanitization**: Filenames derived during vault adoption (`ClientPairingMode::AdoptIntoDir`) are sanitized against reserved Windows DOS device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) in `sanitize_vault_filename`, preventing filesystem namespace collisions and denial-of-service conditions during initial pairing.

### 5.14 Ephemeral QR-Code Optical Pairing & Out-of-Band Transit Isolation (Protocol `YQR2`)
Zero-knowledge optical QR-code pairing establishes a single-use authenticated transit tunnel between devices using an air-gapped optical pre-shared secret:
1. **Optical Secret Generation**: The host creates a 256-bit CSPRNG secret $S_{\text{optical}} \leftarrow \{0,1\}^{256}$ and a UUIDv4 session identifier $\text{ID}_{\text{session}}$. The optical payload encodes `yntrapair://v2?id=<session_id>&s=<secret_hex>&ip=<ip>&p=<port>&sas=<sas>&name=<name>`. Master passwords and database blobs are never encoded in the optical image.
2. **Deterministic Short Authentication String (SAS)**:
   $$\text{SAS} = \text{BLAKE3}_{\text{keyed}}\left(S_{\text{optical}}, \text{"yntra-qr-sas-v2"}\right)[0..8] \pmod{10000}$$
   Formatted as a 4-digit decimal number (`0000`–`9999`) displayed simultaneously on both host display and client viewfinder, providing out-of-band visual MitM resistance.
3. **Transit Key Derivation**:
   $$(K_{\text{c2h}}, K_{\text{h2c}}, K_{\text{auth}}) = \text{HKDF-SHA512}\left(\text{IKM}=S_{\text{optical}}, \text{salt}=\text{"yntra-qr-transit-v2"}\right)$$
4. **Mutual Pre-Authentication Handshake (`YQR2`)**:
   - Client sends 68-byte frame: $[\text{"YQR2"} \mathbin{\Vert} \text{ID}_{\text{session}} \mathbin{\Vert} C_{\text{client}} \mathbin{\Vert} \text{HMAC}(K_{\text{c2h}}, C_{\text{client}})]$.
   - Host validates $\text{ID}_{\text{session}}$ and evaluates HMAC in constant time (`subtle::ConstantTimeEq`).
   - Host responds with 48-byte frame: $[C_{\text{host}} \mathbin{\Vert} \text{HMAC}(K_{\text{h2c}}, C_{\text{host}})]$, verified in constant time by the client before continuing.
5. **Dynamic Session AAD Binding**:
   $$\text{AAD}_{\text{transit}} = \text{"yntra-qr-transit-v2:"} \mathbin{\Vert} \text{ID}_{\text{session}}$$
   Ciphertext is encrypted using $K_{\text{h2c}}$ with `XChaCha20-Poly1305` and a 24-byte random nonce, cryptographically binding every transit packet to the unique session ID.
6. **Zero-IPC Plaintext Isolation**:
   Provisioned master passwords are held exclusively within `zeroize::Zeroizing` buffers in Rust `AppState.pending_adopted_vault`. Plaintext credentials never cross the Tauri IPC boundary into webview JavaScript, preventing memory retention in V8 heap or browser snapshots. Biometric envelopes are wrapped directly into platform hardware (TPM 2.0 / DPAPI / Secure Enclave).

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
| P2P Mutual Auth | Client-first challenge-response HMAC verification | `crates/core/src/services/sync/mod.rs` |
| Zero-Knowledge Pairing | Ephemeral Argon2id transit subkeys + 6-digit PIN | `crates/core/src/services/sync/pairing.rs` |
| Timing-Safe Discovery | `subtle::ConstantTimeEq` across all UDP beacons | `crates/core/src/services/sync/mod.rs` & `pairing.rs` |
| Adopt Mode Isolation | Zero local reads and collision avoidance on unauthenticated clients | `crates/core/src/services/sync/pairing.rs` |
| UDP Amplification Defense | 1:1 request/reply ratio with Argon2id token gating | `crates/core/src/services/sync/pairing.rs` |
| Self-Echo Isolation | In-loop local IP dropping and continuation | `crates/core/src/services/sync/mod.rs` |
| Pairing Socket Release | 40ms `AtomicBool` polling + `cancel_pairing_host` | `crates/core/src/services/sync/pairing.rs` & `src-tauri/src/commands/sync.rs` |
| DOS Device Sanitization | Windows reserved name neutralization on adopt | `crates/core/src/services/sync/pairing.rs` |
| Hardware 2FA KEK | Argon2id 256MB key stretching | `crates/crypto/src/hardware2fa.rs` |
| Constant-Time Verification | `subtle::ConstantTimeEq` comparisons | `crates/core/src/totp/mod.rs` & `crates/cli/src/ipc.rs` |
| Emergency Kit PRF Checksum | Keyed HMAC over session $K_{\text{hmac}}$ | `crates/core/src/vault/emergency.rs` |
| eTLD+1 ccTLD Isolation | Multi-part ccTLD public suffix resolution | `crates/core/src/smartlogin/discovery.rs` |
| CDP JS String Escaping | `serde_json::to_string` DOM serialization | `crates/core/src/smartlogin/engine.rs` |
| CSV Formula Defense | CWE-1236 quote-prefixing on export | `crates/core/src/vault/import_export.rs` |
| Settings Memory Scrubbing | Reset `self.data.settings` on `lock()` | `crates/core/src/vault/manager.rs` |
| Keyfile Factor Isolation | Strict exclusion from browser `localStorage` | `src/pages/Login.tsx` & `CreateVaultModal.tsx` |
| Atomic Wrap Key Creation | `OpenOptionsExt::mode(0o600)` on Unix | `crates/crypto/src/tpm.rs` |
| QR Zero-Knowledge Pairing | Optical CSPRNG secret + YQR2 mutual HMAC | `crates/core/src/services/sync/pairing.rs` |
| Transit Dynamic AAD | `format!("yntra-qr-transit-v2:{}", session_id)` | `crates/core/src/services/sync/pairing.rs` |
| Zero-IPC Secret Isolation | `PendingAdoptedVault` zeroized in native state | `src-tauri/src/commands/sync.rs` |
