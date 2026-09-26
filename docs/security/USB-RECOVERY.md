# USB binding and recovery v2

Implemented in 0.2.4. Existing vaults remain in their original format until recovery v2 is enabled. This is a local file protection feature, not protection against a compromised operating system.

## What USB binding does

On Windows, Settings → Security → USB binding and recovery lets you create a recovery kit and bind that vault copy to an attached USB storage device. The creation dialog also offers USB binding and presents the recovery shares before completing setup.

Opening a bound file requires its password, any configured key file, and a matching connected USB device. Files may be renamed, placed in any directory, or copied to the computer and back. There is no hidden key file on the stick. The stick need not contain the vault while it is being opened.

The identifier comes from the physical drive's manufacturer serial number, not the volume serial, drive letter, partition table, filesystem, Windows account, or vault path. Microsoft describes this field in [Win32_DiskDrive.SerialNumber](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-diskdrive#serialnumber). Ordinary filesystem formatting does not intentionally change this hardware field. That is a design expectation, not a guarantee for every USB controller, driver, firmware update, adapter, or Windows installer tool. A device that stops reporting the same usable serial requires recovery. Missing, generic, or simultaneously duplicated identifiers cannot be enrolled.

Formatting and writing installation media can erase the **vault itself**. Keep a verified backup outside that USB before doing either. Recovery shares restore access to an existing encrypted file; they cannot recreate deleted data.

## Security boundary

The serial participates in password-key derivation and authenticated encryption; binding is not just a removable UI check. Copying the files and knowing the password is insufficient without the matching identifier or two recovery shares. However, a serial number is public, enumerable and spoofable. An attacker who obtains it can implement the same derivation without possessing the physical drive. Malware controlling the computer can read the identifier, capture passwords, steal recovery shares, use an unlocked session or extract keys. This feature does not defeat a RAT or keylogger and does not generate fake keystrokes.

A real non-exportable hardware factor requires a compatible security token and a separately designed hardware-backed unlock protocol. USB serial binding is not equivalent to such a token.

Removing the stick does not erase keys from an already unlocked session or automatically lock it. Use the vault's normal lock controls. Recovery shares and unlocked linked devices are intentionally other routes to the data; protect them accordingly.

## One recovery system

Recovery v2 generates a random 256-bit secret independent of the master password, split into three shares. Any two different shares from the same kit restore that local vault copy. A kit is tied to a replica and generation, with checksums to detect transcription errors. Checksums do not replace cryptographic authentication.

Save each share separately, away from the vault and the other shares. The app exports one share per file, refuses to overwrite an existing export, and asks you to confirm that at least two have been stored. Generating another kit requires current authentication. It replaces the active kit and rotates the local storage key. Generating many kits does **not** create many simultaneously valid recovery routes for the latest file.

Replacing or revoking a kit cannot revoke it for old file copies or backups. Keep old backups as sensitive as the credentials and shares that could open them. Replacing the kit is allowed while USB binding is enabled; removing the last recovery route is blocked until USB binding is disabled.

To recover, choose recovery at login, enter two shares and choose a new password of at least 12 characters. Recovery removes the USB/key-file requirement and consumes that kit for the rewritten file. Create a new kit and explicitly enroll the USB again. It never displays the old master password.

Password changes and USB replacement/removal preserve the current v2 kit. Each newly paired replica creates its own kit if desired. Legacy plaintext-password shares are accepted for legacy password-only vaults; legacy key-file/hardware factors still need their original unlock flow. Legacy shares cannot recover a migrated v2 vault.

## Linking and synchronization

Each replica has its own randomly keyed storage envelope and local password/USB/recovery settings. Pairing shares the inner synchronization keys through the authenticated encrypted channel, without sending USB wrapping slots or recovery material. A paired phone receives an independent local envelope without the desktop's USB requirement. QR pairing can provision a new local password for the protected replica.

P2P and normal WebDAV synchronization exchange encrypted content snapshots. Applying a phone edit on the desktop retains the desktop envelope and recovery kit. Desktop edits likewise preserve the phone's local policy. Two-way entry edits do not require repeated adoption. QR pairing creates new copies; PIN pairing can merge existing vaults and re-encrypt their entries correctly. A new-copy adoption refuses to overwrite an existing file.

The **first migration** changes the inner vault encryption key. Update both peers and re-pair existing devices once; old sync snapshots are no longer readable by the migrated replica. Do not overwrite an old remote snapshot blindly. Resolve or retain unsynchronized edits before migration. USB enrollment/changes after migration and v2 password changes do not change the shared synchronization keys. A local password change is not a revocation of already linked devices.

Legacy raw file-download routes refuse to overwrite a protected local file. USB-bound desktop files cannot be opened directly on Android/Linux using native serial discovery; use pairing to make a local replica, or recovery. PIN/QR host pairing still uses the existing password-based host-open API and does not accept a key-file selection; key-file users must retain that limitation in mind when linking.

## Compatibility

- USB enumeration/enrollment is implemented for Windows. Recovery v2 and independent replica envelopes are platform-neutral Rust, but live mobile/Linux builds were not verified for this change.
- Older applications cannot open `YNS2` files. Keep an appropriate backup before migration and update paired clients.
- Migrating clears biometric quick unlock. Legacy hardware 2FA must be disabled in an already authenticated session before migration. Protected v2 replicas reject biometric and legacy hardware-key enrollment rather than introducing an alternate path around local protection.
- Existing v1–v5 `YNTR` parsing remains for unmigrated files and encrypted synchronization payloads. Recovery remains an offline operation.

## Implementation and verification

The `YNS2` outer layer uses XChaCha20-Poly1305 with authenticated framing and bounded header/blob sizes. Argon2id with the existing hardened parameters derives a password wrapping key from the password, optional length-framed key-file bytes and optional normalized serial. The wrapped package contains a random local storage key and inner subkeys. Recovery wraps equivalent material under its independent secret. Temporary secret buffers are zeroed on drop. Saving stages and flushes ciphertext before atomic replacement; a failed recovery or binding transition restores the in-memory unlock state.

Shamir multiplication uses fixed rounds without secret-indexed lookup tables and samples the full coefficient field, including zero. Parsing rejects oversized, malformed, duplicate and mixed-kit shares. Recovery rotation changes the outer payload key, preventing old recovery material from opening a new payload by substituting its old header.

Regression coverage includes wrong factors, tampering, malformed framing, recovery replacement/consumption, failed-save rollback, independent replica sync in both directions, decrypted entry contents after cross-vault pairing, and separate share export confirmation. A real connected Windows USB was tested non-destructively with a disposable vault: enrollment, lock/reopen, rename/copy to the computer and back, password change retaining binding, and recovery with the original kit passed. No existing user vault was opened or changed. Formatting, unplug/replug, Windows reinstallation, other physical computers and firmware changes were not exercised. Automated tests do not constitute an independent cryptographic audit.

Windows validation including the subsequent reliability/UI fixes: 285 Rust workspace tests passed (nine opt-in tests ignored), 127 frontend tests passed, and the TypeScript/Vite production build passed. The live USB probe passed during the recovery implementation. These counts record that implementation milestone; later update checks are described in [Updates and application data](UPDATES.md). Real phone-PC synchronization and platform-specific device behavior still need device testing.

CLI usage: `yntra --path <file> recovery generate --output-dir <directory>`, `recovery revoke`, and `recovery restore --share-a-file <file> --share-b-file <file>`. Password prompts are hidden. Move generated share files to separate safe locations. The old `shamir` diagnostic command produces legacy hash shares and is not a v2 recovery kit.
