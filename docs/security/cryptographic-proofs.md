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
* `yntra-vault-search-index-key-v1`: Blind search indexing

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
The remote API receives $16^5 = 1{,}048{,}576$ possible prefixes. The server cannot determine which password within that prefix cluster was tested, and the complete hash never leaves the local machine.

---

## 6. Audit Verification & Compliance Checklist

| Security Property | Mechanism | File Location |
| ----------------- | --------- | ------------- |
| KDF Resistance | Argon2id ($m=256\text{MB}, t=4, p=4$) | `src-core/src/crypto/kdf.rs` |
| Subkey Separation | HKDF-SHA512 with distinct info tags | `src-core/src/crypto/kdf.rs` |
| Envelope AEAD | XChaCha20-Poly1305 + Header AAD | `src-core/src/crypto/cipher.rs` |
| Field-Level AEAD | XChaCha20-Poly1305 / AES-256-GCM | `src-core/src/crypto/cipher.rs` |
| RAM Locking & Canary | 3-Page `LockedBuffer` with `PAGE_NOACCESS` | `src-core/src/crypto/locked_buffer.rs` |
| Volatile Zeroization | `ZeroizeOnDrop` + `write_volatile` | `src-core/src/crypto/mod.rs` |
| Heap Scrambling | Ephemeral XChaCha20-Poly1305 | `src-core/src/crypto/scrambled.rs` |
| Atomic File Writes | Temp file `write()` + atomic `rename()` | `src-core/src/vault/manager.rs` |
| k-Anonymity Query | 5-char SHA-1 prefix over HTTPS | `src-core/src/breach/hibp.rs` |
