# Updates and preservation of application data

Updated for 0.2.4, 2026-09-26. These changes do not publish a release or change the installed app's version.

## Checking and installing

Automatic checking is opt-in. When enabled, one check runs after app startup, including on the vault-selection/login screen. It no longer depends on opening Settings. Airgap mode suppresses the automatic check. A manual check is an explicit network action. No platform installs an update without the user's action.

The official channel fetches `latest.json` from the YntraAB/yntra-vault GitHub release, with the GitHub releases API as a fallback. Checks use HTTPS, a timeout and a bounded response. Package downloads are also time/size bounded. Stable installations are not offered prereleases. Invalid versions, mixed release/package versions, malformed checksums and asset URLs outside the versioned official release repository are rejected. A missing compatible asset is displayed as unavailable, not installed successfully.

| Platform | Update behavior and verification |
| --- | --- |
| Android | Re-fetch official metadata, download and verify SHA-256, atomically stage a content-addressed APK in private cache, recheck its digest, application ID, installed signing certificates, release version name and strictly greater versionCode, then ask Android's package installer to perform the update. Android enforces APK signing. |
| Windows portable | Re-fetch official metadata, verify SHA-256, stage/flush the complete file, retain the old executable and replace in place with rollback if replacement fails. Restart is required. The portable filename determines this mode; renamed binaries can fall back to the ordinary download flow. |
| Windows installer, Linux, macOS | Open the corresponding official release asset in the browser. The application does not claim that download opening proves package verification or successful installation. Use the downloaded installer/package to update the existing installation. |

Checks and installation preparation cannot overlap in the shared UI. Native preparation also has its own exclusive guard. On Android, permitting installation from this source may require a trip to system settings and a retry. Errors remain visible in the update dialog. “APK ready” means that the system installer was opened; cancellation does not mean the update was installed. The next app launch reads the version of the actual running build.

**Security boundary:** SHA-256 delivered through the same HTTPS release channel is integrity checking, not independent publisher authentication. Desktop/CLI/portable packages still lack detached updater signatures with a pinned verification key. Compromise of the release account/channel remains a desktop update risk. A stronger future channel requires a separately protected signing key, signatures over versioned package metadata, a pinned verification key in clients and an explicit key-rotation policy. The Android installer additionally requires the installed application's signing identity; Android key rotation would need a deliberately reviewed compatibility change because the current check requires the same signer set.

## What is retained

The application ID stays `com.yntravault.app`. The WebView keeps its existing `main` window label, default data directory and HTTP custom-protocol origin. Changing an origin can strand WebView storage, as described in [Tauri's useHttpsScheme configuration](https://v2.tauri.app/reference/config/#usehttpsscheme).

The following metadata is additionally stored as `ui-metadata-v1.json` in Tauri's application data directory, outside the WebView cache:

- Up to ten recent vault IDs, display names and full file paths.
- App preferences, including language, layout sizes, auto-lock and update-check preferences.
- Theme and setup-completion state.

At first launch after upgrading, the app migrates the existing four localStorage keys without renaming them. Later launches restore native metadata before React providers read settings or choose a vault; native values win over stale browser values. Writes are serialized, bounded, flushed and atomically replaced under an operating-system file lock. Per-key updates prevent a settings save from overwriting a different instance's vault list; simultaneous edits to the same key remain last-writer-wins. An unreadable or unsupported metadata file is reported instead of overwritten with an empty first-run state. Failed writes remain queued and produce a notification; the app's update action retries pending saves and proceeds only if they succeed.

This is a non-secret metadata file protected by the user's application-data permissions, not a second vault. It excludes master passwords, keyfile paths, recovery shares and session keys. Vault names/paths and ordinary settings are visible to software that can already read the user's application data. The encrypted vault files stay in their existing locations. Linked-device identity remains in its existing native `device-id` file; trusted-device records remain in the vault. Browser caches and temporary discovery addresses are not part of the four-key metadata store.

Moving a vault or changing a removable drive's letter still requires selecting its new location. An update cannot restore a deleted vault, missing USB drive, erased app data or lost operating-system profile. Native metadata is an additional preservation mechanism, not a vault backup. A WebView cache reset before the first migration cannot be repaired from metadata that has not yet been created.

## Android upgrade identity

Android updates require compatible application ID/signing identity; see [Android's update requirements](https://developer.android.com/google/play/app-updates). The release workflow uses the existing permanent signing key and now rejects APKs with a different public certificate fingerprint, package ID, version name or versionCode. Its version-code mapping remains `major * 1000000 + minor * 1000 + patch`; minor/patch values must stay below 1000. Published releases must increase versions instead of replacing binaries under an unchanged version.

An in-place update does not uninstall the app, clear storage or move vaults. Older installations signed with historical temporary keys cannot be upgraded in place using the permanent key. The updater refuses that mismatch and never uninstalls automatically. Export and verify encrypted vault backups before any deliberate migration. Android cloud backup/device transfer remains disabled for vault privacy; this is separate from preserving data during an in-place package update.

## Verification and remaining device work

Regression coverage includes WebView-to-native migration, restoration with empty/unavailable browser storage, exact vault-path retention, serialized writes, deleted-recent-entry retention, failed-read/write handling, exclusion of unlock factors, damaged/newer metadata preservation, startup checks, overlapping operations, installer errors/retry, missing checksums, official asset/version validation, immutable APK staging, and Android release identity checks. The frontend production build and Rust workspace tests are separate from a real package upgrade.

Release 0.2.4 validation: 145 frontend/tooling tests passed (684 assertions); the complete optimized Rust workspace suite passed 294 tests with 9 opt-in tests ignored. The production frontend, Windows NSIS/MSI and CLI builds passed locally in a separate target directory without closing the running app. [The release workflow](https://github.com/YntraAB/yntra-vault/actions/runs/36261008729) successfully built Windows, Linux, macOS Apple Silicon and the Android universal APK, including permanent certificate/application/version validation. Build Tools 37 certificate labels are supported with the same pinned identity.

The real Rust updater check against the published 0.2.4 release passed for Android, Windows installer/portable/CLI and Linux: older versions receive 0.2.4 and the current version receives no unchanged-version update. Release URLs, version and checksums were verified. These checks did not install packages or open user vaults.

Remaining device validation: run an Android upgrade on a device with the existing permanent-signed app: record its vault path, language/layout and linked-device identity; install the newer APK in place; verify those values and encrypted content, unlock and synchronize again. Also exercise installer cancellation, permission denial/grant/retry, low storage and process termination during preparation. Android compilation and signing passed in CI; actual phone installation and in-place data preservation remain unverified on a physical device. Unchanged-version rebuilds are intentionally not offered as upgrades.
