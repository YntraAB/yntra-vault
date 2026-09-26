# Emergency recovery specification: v2 and legacy compatibility

Implementation: 0.2.4. Source: [emergency.rs](../../crates/core/src/vault/emergency.rs), [storage.rs](../../crates/core/src/vault/storage.rs), and [sharing.rs](../../crates/crypto/src/sharing.rs). For user steps and limits, see [USB binding and recovery](USB-RECOVERY.md).

## What recovery restores

New kits split an independent random **256-bit recovery secret**, not the master password. Two distinct compatible shares plus the encrypted vault file unwrap that replica's protected key material and establish a new password. They cannot recreate a deleted vault. Recovery deliberately clears unavailable USB/keyfile requirements and consumes the active kit; generate a new kit and re-enroll desired factors afterwards.

Legacy kits shared password material. Their decoding remains a compatibility path for old files, not the scheme used for new kits. Never describe recovery v2 as displaying or reconstructing the original password.

## Threshold sharing and framing

The random secret is split 2-of-3 over GF(256), using polynomial `0x11B` and evaluation points 1, 2 and 3. Each byte uses a uniformly random coefficient from the full field, including zero. Any two distinct evaluation points reconstruct the secret; a single raw Shamir share does not determine it. Metadata/checksums and the wider application are not covered by a blanket information-theoretic security claim. The implementation avoids secret-indexed multiplication tables.

Each exported v2 record has this form (placeholders only):

```text
YNTRA2:<replica UUID>:<kit UUID>:<encoded Shamir share>:<checksum>
```

The checksum is the first eight bytes of BLAKE3 over the preceding framed body, encoded as sixteen lowercase hex characters. It detects transcription/corruption errors; it is not a signature or proof of ownership. Decoding bounds input and validates structure, UUIDs, checksum and 32-byte share payload. Reconstruction rejects duplicate share indices and mixed replica/kit identities. The encrypted recovery slot must also authenticate successfully.

The generator reconstructs a share pair and compares it with the generated secret before returning the kit. Transient buffers use zeroizing wrappers; debug output redacts share values. GUI display/export still crosses the frontend/OS boundary, so this is not a guarantee against a compromised host or all plaintext memory exposure.

## Authentication, rotation and local policy

- Generation and revocation require verification of the current password and any selected keyfile. Invalid authentication aborts without mutating the kit.
- First migration installs the authenticated `YNS2` local envelope, clears biometric enrollment and requires device re-pairing. Hardware-key enrollment and this recovery/USB mode cannot currently be combined.
- Each replacement kit has a fresh generation UUID and rotates local storage protection. Revocation likewise changes local protection. Old shares stop opening the updated file but may still open old backups; offline snapshots cannot be revoked retroactively.
- Ordinary password/USB changes preserve the active v2 kit. State transitions roll back in memory if saving fails; stale sessions cannot overwrite a newer recovery header.
- Recovery audit history lives encrypted in the vault. Its active `verification_hash`/fingerprint field contains the v2 kit UUID, not a hash of the master password. Local audit/wrapping records are omitted from sync snapshots.
- GUI setup shows one share at a time and requires two distinct saved-share confirmations. Native export writes a selected share to a separate document and refuses existing content. Never keep all shares together with the vault or put real shares in documentation/tests/logs.

## Entry points

Core methods include `generate_emergency_kit_with_keyfile`, `revoke_emergency_kit` and `recover_with_shares`. Their current definitions in the source are authoritative; password/keyfile handling differs between native document URIs and ordinary file paths.

The typed native/frontend contract is [src/types/ipc.ts](../../src/types/ipc.ts), implemented by [native auth commands](../../src-tauri/src/commands/auth.rs). It includes generation, selected-share export, recovery revocation, protected vault recovery and local-protection status. Resetting an audit view must never substitute for cryptographic revocation. The CLI exposes `recovery` generation/revocation/restore operations; use its current help for arguments instead of a historical `recover` command example.

## Security and validation limits

A recovery kit is an intentional alternative route to access, including when a USB factor is unavailable. Protect two-share access as carefully as an unlocked vault. Public USB serial binding does not defeat identifier spoofing or a RAT; separately supported hardware security keys use a different protection model.

Tests cover share reconstruction, malformed/mixed/duplicate shares, authentication, rotation/revocation, old-backup behavior, password changes, rollback and protected desktop/mobile synchronization. They do not establish universal USB-controller stability or successful Android document-provider/device behavior. See [the update guide](UPDATES.md) for latest aggregate verification and [the USB guide](USB-RECOVERY.md) for recovery-specific test limits. No independent security audit is implied.
