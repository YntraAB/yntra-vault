# Security changes in 0.2.5

0.2.5 is a security and reliability update in preparation. The source review, regression tests and dependency checks do not establish that every vulnerability has been found. New feature proposals are deferred.

## Passwords, USB and recovery

Changing the password or USB factor of a protected `YNS2` vault rotates both the local storage key and the recovery secret. Previously, retained wrapping material in an old file could remain useful against a later local payload. The app now presents replacement shares and requires confirmation that two have been saved. The share screen can be cancelled with a warning after the change has committed; a new kit can be generated from Security → Access and recovery. Old passwords and old kits can still open old backups; no local change can erase copies already held elsewhere.

Ordinary linked replicas keep their shared inner data keys. A password change affects the local copy and is not a way to revoke a linked device. See [USB and recovery](USB-RECOVERY.md).

## Devices and automated credentials

P2P synchronization authenticates each installation with an individual Ed25519 identity and establishes fresh session keys using ephemeral P-256 agreement bound to both signatures. Trusted identities belong to the local replica and cannot be restored by an incoming trust list. An empty list grants no access. Older devices must update and explicitly re-pair. Losing the installation's protected identity file also requires re-pairing.

Removing a device prevents future authenticated sessions with that replica. It does not delete the removed device's local database or make old shared encryption material unknowable. Revoke any separate WebDAV credentials and access grants when removing an untrusted device. Cryptographic recovery from an already compromised device requires migrating the still-trusted data to fresh shared keys; this update does not claim that deleting one device performs that migration.

Git credential requests require a matching HTTPS origin and account, with ambiguous matches refused. Smart Login uses parsed origins and explicit service-to-identity-provider pairs. It no longer trusts arbitrary sibling sites under a shared hosting suffix. Insecure remote redirects and user-info URL ambiguity are refused. These narrower rules may require manual login for a service that uses an unlisted identity provider.

## Locking and network privacy

Entry and tag operations are tied to the session that started them. A late response or failed-operation rollback cannot repopulate the UI after lock or a vault switch. List previews exclude decrypted passwords, notes, TOTP secrets, recovery codes, sensitive custom fields and staged attachment data.

Website icons remain an optional external request and are enabled by the existing default. Their domain/image cache now exists only in memory and is cleared on lock; legacy app-owned persistent caches are removed. Icon providers can still observe requested domains when icons are enabled. Application metadata still includes recent vault names and paths; it is not encrypted vault content.

Closed System mode is enforced by native network gates as well as UI controls. The desktop starts with network access denied until settings are loaded; changing the mode cancels pending optional network operations. Cancellation cannot retract a request already transmitted or stop unrelated programs. The CLI's explicit network commands use its own process policy.

Breach checks send only five SHA-1 prefix characters, use padded responses, impose time/size limits and treat zero-count padding as a non-match. Previously checked results become eligible for refresh. These choices follow the [Pwned Passwords API](https://haveibeenpwned.com/API/v3#PwnedPasswords).

## Local storage and platform behavior

Plaintext exports reject the open vault and aliases of it. Saves use an operating-system lock and compare the disk revision before atomic replacement, preventing another open instance from silently overwriting a newer file. Vault writers enforce the same size budget as readers; staged attachments follow the 25 MiB limit.

Protected memory uses the host page size and reports protection/locking failures. Sensitive native objects clear buffers on drop. JavaScript and OS/runtime copies cannot be guaranteed to be physically zeroed. Clipboard clearing respects ownership so unrelated copied text is preserved. Tests that use a real clipboard, authentication store, network service, browser or device are opt-in.

OS screen-lock integration is shown only on supported platforms. Biometric unlock requires actual platform consent; unsupported consent does not silently succeed. Password unlock remains available. Environment variables cannot enable the release build's test-only biometric/hardware mocks.

Removable-storage presence is fail-closed while a protected vault is unlocked. The native desktop watcher checks both the exact vault session/file and the bound USB serial. Losing either locks the session, cancels tracked network and autotype work, clears favicon/clipboard state through the normal lock path, and returns the UI to the locked screen. A 250 ms recheck permits an atomic save replacement; stale temporary files do not keep a missing vault open. The application remains open so the user can reconnect and unlock again.

## Dependency maintenance

Dependency remediation includes rustls 0.23.45, ratatui 0.30.2 (with lru 0.18.5), updated frontend dependencies and esbuild 0.28.2. GTK3 still requires glib 0.18.5; the checked-in [glib patch record](../../vendor/glib/YNTRA-PATCH.md) documents the upstream mutable-pointer fix for RUSTSEC-2024-0429. Version-only scanners may continue to flag that backported package.

The recorded dependency checks found no Bun advisories after these updates. Rust advisory results still include maintenance notices for bincode, proc-macro-error and five UNIC components. Those notices remain tracked; a clean vulnerability scan is not proof that dependencies are free of defects. Replacing serialization dependencies must preserve compatibility with existing vaults.

## Updates and release boundary

See [updates](UPDATES.md) for the pinned publisher key, detached signed metadata, package verification and expiry behavior. Build jobs have restricted permissions, actions are pinned, and signing runs in separate jobs. The protected signing environment and secrets must be configured before publishing 0.2.5. Old unsigned 0.2.4 metadata is deliberately rejected by a 0.2.5 client.

This source change does not publish a release or establish a successful installed upgrade. Physical USB behavior, phone-PC pairing, Linux/macOS-specific behavior and final signed package upgrades require platform validation. The existing application ID and Android signing identity are retained.
