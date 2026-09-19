# Changelog

All notable changes to Yntra Vault will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2026-09-19

### Added
- **P2P Local Network & VPN Warning Modal**:
  - Implemented `P2pVpnWarningModal` matching the Smart Login modal design system, alerting users before Wi-Fi pairing that active VPN connections block local network (LAN) discovery.
  - Added "Don't show this warning again" preference persisted to `localStorage` (`yntra-vault-p2p-vpn-warning-dismissed`).
  - Added flow isolation: closing or canceling the initial VPN notice exits pairing without advancing into "Pair Device via Wi-Fi".
  - Added discrete header shield button (`ShieldAlert`) and neutral reminder banner in `DevicePairingWizard` allowing users to review VPN guidance on demand.
  - Formatted with concise copy and complete translation key parity across Swedish and English.
- **Delete Unused Tags Confirmation Modal**:
  - Created `DeleteUnusedTagsModal` matching `DeleteTagModal` 1:1 in design, typography, and keyboard safety, preventing accidental bulk deletion when selecting "Delete Unused Tags" from the sidebar context menu.
  - Supports both single and plural tag previews with color-coded badges, safe default focus on Cancel, Tab cycling, and Escape dismissal.
  - Added differentiated toast feedback distinguishing single vs multiple tag deletions.
- **Android Native Camera & WebChromeClient File Chooser Bridge**:
  - Implemented custom `MainActivity.kt` in `src-tauri/android-overrides` overriding `onPermissionRequest` to request OS-level runtime camera permissions instead of silently denying WebView access.
  - Implemented `onShowFileChooser` in `MainActivity.kt` allowing users to pick QR images or photos directly from device galleries or system photo pickers.
  - Added automated build hook (`scripts/apply-android-customizations.js`) integrated into `release.yml` and `package.json`.
- **High-Performance Multi-Scale QR Decoder**:
  - Created `src/utils/qrDecoder.ts` featuring hardware-accelerated `BarcodeDetector` with dynamic downscaling (1024px, 640px, 1600px) to decode high-resolution mobile camera captures (12MP–48MP) without memory exhaustion.
  - Added bidirectional contrast inversion (`inversionAttempts: 'attemptBoth'`) and adaptive binarization for photos of screens or low-light conditions.
- **Direct Camera Scanner in TOTP / 2FA Setup**:
  - Integrated `QrScannerModal` directly into `EntryModal`, allowing users to scan 2FA QR codes live from screen or paper without manually typing base32 secrets.

### Changed
- **Mobile Entry Detail Tag Navigation**:
  - Updated tag click handler in `PasswordDetail` on mobile devices (`< 768px`) to clear `selectedEntry` when filtering by tag, immediately closing the full-screen detail view and transitioning the user to the filtered entry list.
- **Android App Sandbox Key Wrapping Preparation**:
  - Added Android app-sandbox private data paths (`/data/user/0/com.yntravault.app/files`, `/data/data/com.yntravault.app/files`) in `crates/crypto/src/tpm.rs` for isolated key wrapping while strictly enforcing Invariant 25 (rejecting insecure `/tmp` paths).
  - Explicitly guarded Android biometric unlock against unprompted access in `crates/crypto/src/biometric.rs` pending active native BiometricPrompt bridge integration, preventing unauthenticated auto-unlock.

### Fixed
- **Ghost Windows Hello / Biometric Prompt on Application Close**:
  - Resolved issue where closing the window (or minimizing to tray) triggered an unwanted Windows Hello modal over the desktop.
  - Added window visibility guards in `Login.tsx` (`document.hidden`, `visibilityState !== 'visible'`) and added listeners to only prompt when the application window is actively restored and focused.
  - Added `window.is_visible()` validation in `unlock_vault_biometric` (`src-tauri/src/commands/auth.rs`) to prevent invoking system biometrics when the window is hidden.
  - Resolved tray restore race condition by resetting `autoBioTriggered.current` ref when hidden so focusing/restoring immediately prompts for biometrics.
- **Android WebChromeClient Least Privilege**:
  - Enforced strict `RESOURCE_VIDEO_CAPTURE` resource granting in `MainActivity.kt`, rejecting audio capture and preventing WebView crashes from unrequested OS permissions.
- **Smart TOTP QR URI Parameter Parsing**:
  - Added automatic extraction and intelligent prefilling of `title` (issuer) and `username` (account) when scanning standard `otpauth://` QR codes in `EntryModal`, alongside automated base32 space/dash cleaning.
- **Android WebView Media Stream CSP**:
  - Added `media-src 'self' blob: data: mediastream:;` in `tauri.conf.json` to allow live camera feeds in Android WebViews.

### Security
- **Android App Sandbox Key Wrapping & Invariant 25 Enforcement**:
  - Restricted hardware key wrapping paths on Android strictly to internal app-isolated storage directories (`/data/user/0/com.yntravault.app/files`, `/data/data/com.yntravault.app/files`), rejecting world-writable or shared storage locations.
- **Biometric Prompt Window Visibility & Background Suppression**:
  - Enforced active window visibility verification (`window.is_visible()`) prior to launching native biometric authentication, preventing background or tray-minimized Windows Hello prompt execution.

## [0.2.1] - 2026-09-19

### Added
- **Multi-Tier Favicon Resolution Pipeline**:
  - Implemented 4-tier fallback resolution in `crates/core/src/services/favicon.rs`: (1) DuckDuckGo Favicon CDN (`https://icons.duckduckgo.com/ip3/{domain}.ico`) for fast, crisp ICOs, (2) Google s2 API (`https://www.google.com/s2/favicons?domain={domain}&sz=64`) for broad global coverage, (3) Direct host fallback (`https://{domain}/favicon.ico`), and (4) Parent/base domain fallback for subdomains without dedicated root icons (e.g. `login.live.com` -> `live.com`, `app.slack.com` -> `slack.com`).
- **Two-Tier Persistent Favicon Caching**:
  - Frontend persistent storage in `localStorage` (`yntra-favicons-cache`) caching up to 250 data URIs for instant 0ms startup rendering without IPC or network overhead.
  - Backend persistent disk caching in OS local cache directories (`LOCALAPPDATA/Yntra Vault/cache/favicons` on Windows, `~/.cache/yntra-vault/favicons` on Unix/macOS) storing fetched data URIs across application restarts.
- **Intelligent Title Fallback for Domains**:
  - Added domain parser fallback extracting domains from entry `title` when the `url` field is empty or missing (e.g. entries titled `github.com` or `reddit.com`).
- **Network Recovery Auto-Retry**:
  - Registered window `online` event listener to automatically clear failure cooldowns and re-fetch missing favicons as soon as poor network connectivity recovers.

### Changed
- **Concurrency Throttling & Socket Starvation Prevention**:
  - Enforced bounded concurrency on both frontend (`MAX_CONCURRENT_FETCHES = 4`) and backend (`tokio::sync::Semaphore::new(6)`), preventing network socket exhaustion, DNS timeouts, and UI freezes when rendering vaults with large numbers of items.
- **Streamlined Client P2P Adoption on Login Screen**:
  - Clicking "Link via P2P" from the vault selection / login screen starts directly at the connection step with the optical QR scanner mode active by default, removing redundant navigation steps.
  - In manual PIN mode on the login screen, integrated the master password field directly on the same screen alongside the 6-digit PIN input for seamless adoption.
- **Password Input Hygiene & Discrete Tooltips**:
  - Replaced overflowing sentence placeholder text with standard masked dots (`••••••••••••`).
  - Added accessible `ActionTooltip`s to password visibility toggle buttons and helper icons in `DevicePairingWizard`.

### Fixed
- **Favicon Startup Race Condition & Permanent Failure Poisoning**:
  - Resolved race condition where initial entry rendering queried favicons before `set_external_favicons_enabled` completed IPC synchronization with Rust, which previously caused `null` to be permanently cached for all initial entries.
  - Eliminated permanent `null` caching: replaced permanent in-memory failure markers with a 30-second transient cooldown (`failedCooldowns`).
  - Synced cache invalidation with settings: toggling `externalFaviconsEnabled` off immediately purges both in-memory and disk/localStorage caches, while toggling on dispatches reset events to re-fetch.
- **Device Pairing Wizard Progress Stepper Misalignment**:
  - Corrected wizard stepper progress bar to dynamically map over `stepTitles`, eliminating the 4th orphaned blank bar and fixing off-center horizontal alignment.
- **Sidebar Tag Drag-and-Drop Gesture State**:
  - Eliminated stuck shadow and scale artifacts caused by unbacked inline `whileDrag` styles on Framer Motion `<Reorder.Item>`.
  - Guarded tag drop commit with initial-vs-final order diffing and deferred backend persistence to prevent mid-gesture layout interruption.
  - Replaced global body drag attributes with scoped CSS grab cursor states.

### Security
- **Favicon Concurrency, MIME Validation & SSRF Shield (Invariant 31)**:
  - External favicon downloads strictly respect the `EXTERNAL_FAVICONS_ENABLED` gate (disabled by default in Rust core).
  - Rate-limited socket usage (`tokio::sync::Semaphore::new(6)` in Rust, `MAX_CONCURRENT_FETCHES = 4` in React) prevents remote denial of service / TCP starvation under high vault entry counts.
  - Image MIME & magic signature validation: enforces image MIME types and magic headers (PNG, ICO, SVG, WebP, GIF, JPEG) capped at 512 KB, rejecting non-image payloads (such as SPA HTML 200 OK pages) and preventing cache poisoning.
  - SSRF & Local Network Shield: drops and rejects loopback, private LAN (`10/8`, `172.16/12`, `192.168/16`), cloud metadata (`169.254.169.254`), and internal/anonymity TLDs (`.local`, `.lan`, `.internal`, `.home`, `.corp`, `.onion`, `.i2p`) from external resolution.
  - BLAKE3 Cache Key Isolation: hashes domain keys via BLAKE3 for on-disk persistence, eliminating Windows DOS device name collisions (`CON`, `PRN`, `AUX`, `NUL`) and avoiding filesystem path traversal.

## [0.2.0] - 2026-09-19

### Added
- **Zero-Knowledge Optical QR-Code Device Pairing Protocol (`YQR2`)**:
  - Implemented an air-gapped, zero-knowledge pairing exchange allowing clients (smartphones, tablets, secondary workstations) to securely pair and adopt vaults without manually typing IP addresses, 6-digit PINs, or master passwords.
  - Optical payload uses custom URI scheme (`yntrapair://v2?id=<session_id>&s=<secret_hex>&ip=<ip>&p=<port>&sas=<sas>&name=<name>`), encoding a 256-bit CSPRNG ephemeral optical secret, session UUID, host network endpoints, and Short Authentication String (SAS).
  - Short Authentication String (SAS) visual badge (4-digit decimal confirmation code derived via keyed BLAKE3) displayed concurrently on both host screen and scanner interface for out-of-band visual verification against Active Man-in-the-Middle (MitM) attacks.
  - Single-use 90-second ephemeral session TTL with real-time countdown progress indicator and 1-click regeneration.
  - Automatic memory zeroization (`zeroize::Zeroizing`) of ephemeral optical pre-shared keys, transit encryption keys, and adopted credentials immediately following session completion.
- **Embedded Camera QR Scanner with Dual-Engine Optical Fallback**:
  - Built high-performance in-app camera viewfinder modal (`QrScannerModal`) utilizing browser-native `BarcodeDetector` where hardware-accelerated, with seamless fallback to `jsQR` canvas raster scanning.
  - Real-time animated scanning beam, camera switcher (front/rear lens selection), flash/torch detection, and local image file drag-and-drop / picker fallback for environments lacking direct webcam access.
  - Discrete visual feedback upon successful optical capture.
- **Seamless 1-Click Biometric Enrollment on Adopted Devices**:
  - Optional transit master password transfer encrypted end-to-end under the ephemeral optical key (`XChaCha20-Poly1305` + dynamic session `AAD`), enabling mobile and desktop clients to enroll directly into hardware biometrics (Touch ID, Face ID, Android Keystore, Windows Hello) immediately upon adopting a vault without manual master password entry.
  - Master password in volatile memory is cleared and zeroized immediately after biometric key envelope creation.
- **Unified Tabbed Device Pairing Interface**:
  - Re-architected `DevicePairingWizard` with a sleek segmented mode switcher: `QR-kod (Snabbast)` as the modern optical default, alongside `6-siffrig PIN` as the manual fallback.
  - Discrete monochrome SVG QR code renderer (`QrCodeView`) adhering strictly to the design system geometry (`rounded-[3px]`, semantic theme tokens).
  - Synchronous 4-digit SAS verification badge displayed during connection and success states on both host and client screens.
- **Manual Password Adoption Fallback & IPC Command**:
  - Added safe adoption flow for vaults adopted without an optical password (`include_password: false`): client saves into a dedicated `.vdb` file and prompts user to verify the existing master password.
  - Added `complete_adopted_vault` Tauri IPC command to finalize and verify adoption without leaking secrets across the webview bridge.
- **Date-Section Grouping for Password Entries (`groupByDate`)**:
  - Added structured chronological section headers ("Idag", "Igår", "Tidigare" / "Today", "Yesterday", "Earlier") when sorting entries by updated or created date.
  - Added a dedicated setting toggle in `GeneralTab` (`settings.group_by_date`) and a quick-toggle option directly in the list area context menu (`menu.group_by_date`).
  - Enhanced sort order fallback logic to cleanly handle entries with missing timestamps.
- **Refined Smart Login & Autotype Tooltips & Disabled States**:
  - Replaced abrupt button hiding with a discrete disabled button and helpful localized tooltips (`smart_login.disabled_tooltip`, `context_menu.autotype_no_url_tooltip`) when an entry lacks a website URL or application target.
  - Added localized helper descriptions for hardware key enrollment and challenge verification (`hw.enroll_desc`, `hw.test_desc`).

### Security
- **Zero-IPC Master Password Leakage Invariant**:
  - Master passwords provisioned during adoption are held exclusively inside `zeroize::Zeroizing` buffers within Rust Tauri `AppState.pending_adopted_vault`. Plaintext credentials are NEVER serialized or transmitted across the Tauri IPC boundary to webview JavaScript or V8 heap (`receivedMasterPassword` completely eliminated from React state).
  - Hardware biometrics wrap credentials directly in native memory (TPM 2.0 / DPAPI / Secure Enclave), and RAM is purged immediately upon completion.
- **Dynamic Session AAD Binding**:
  - Transit AEAD payload encryption (`XChaCha20-Poly1305`) binds dynamic Additional Authenticated Data (`format!("yntra-qr-transit-v2:{}", session_id)`), cryptographically binding every encrypted packet to the unique session UUID and eliminating cross-session replay or payload injection attacks.
- **Protocol `YQR2` Mutual Pre-Authentication Handshake**:
  - Employs mutual HMAC challenge-response verification (68-byte client request, 48-byte host response) using dedicated `client_to_host` and `host_to_client` subkeys derived via `HKDF-SHA512` over the 256-bit optical CSPRNG secret.
  - Unauthenticated peers or network scanners are rejected immediately before any vault metadata or payload is exposed, closing unauthenticated password and sync oracles.
- **Non-Destructive Adoption & Collision Avoidance (`AdoptIntoDir`)**:
  - Client QR adoption uses `ClientPairingMode::AdoptIntoDir`, adopting into a collision-free file (`<HostVaultName>.vdb`, `<HostVaultName> (1).vdb`) in the user's vaults directory, guaranteeing that existing vaults on disk are never overwritten, unpersisted, or corrupted.
- **256-Bit CSPRNG Optical Pre-Shared Secrets & Out-of-Band Transit Isolation**:
  - The QR code contains zero plaintext passwords, database payloads, or long-term private keys. All Wi-Fi traffic is encrypted using `XChaCha20-Poly1305` AEAD with subkeys derived via `HKDF-SHA512` bound to the optical pre-shared secret.
  - Eavesdroppers on the local Wi-Fi network who observe network packets cannot decrypt the transit payload without optical line-of-sight to the host screen.

### Changed
- **P2P Discovery & Sync Engine Extensibility**:
  - Modularized pairing protocol functions (`run_p2p_qr_pairing_host`, `run_p2p_qr_pairing_client`, `derive_qr_pairing_subkeys`, `compute_qr_sas_code`, `complete_adopted_vault_save`) in `crates/core/src/services/sync/pairing.rs`.
  - Updated desktop Tauri IPC commands and state management to support concurrent cancelable QR pairing background threads and pending adopted vault storage.
- **IPC Contract Hardening**:
  - Updated `QrClientPairingResult` in `src/types/ipc.ts`, `crates/core`, and `src-tauri`: replaced sensitive `master_password` field with `has_master_password: bool`, `needs_password: bool`, and `sas_code: String`.
- **Localization Parity & Comprehensive Fallbacks**:
  - Added `'pairing.manual_adopt_title'`, `'common.retry'`, `'hw.enroll_desc'`, and `'hw.test_desc'` to `en.ts` and `sv.ts`.
  - Propagated complete translation coverage and fallbacks across all 24 supported language dictionaries.

## [0.1.9] - 2026-09-18

### Added
- **Symmetrical Key File Generation in Master Password Rekeying**:
  - Added option to generate a brand-new cryptographically secure `.key` file or select an existing one directly in `ChangeMasterPasswordModal`.
  - Integrated `saveFileDialog` and `backend.generateKeyFile` to safely write random 32-byte key files with strict permissions prior to cryptographic subkey derivation and re-encryption.
- **CLI Key File Generation & Initialization**:
  - Added `--gen-keyfile <PATH>` option to `yntra init`, allowing new vaults to be initialized and bound to a fresh 32-byte keyfile in a single step.
  - Added `--keyfile <PATH>` to `yntra generate` to create standalone cryptographically secure 32-byte keyfiles directly from the terminal.
- **Global Appearance Configuration in SettingsProvider**:
  - Implemented root-level propagation of user-selected `fontSize` and `data-density` attributes directly inside `SettingsProvider`, ensuring consistent UI typography and density across Login, Onboarding, and Vault Selection screens.
- **Comprehensive Window & Modal Aesthetic Harmonization**:
  - Standardized all application modals and dialogs (`DeleteEntryModal`, `DeleteTagModal`, `CreateTagModal`, `EditTagModal`, `EntryModal`, `BulkEditModal`, `AppPickerModal`, `AttachmentPreviewModal`, `SmartLoginModal`, `Hardware2FaModal`, `BackupTab` manual IP modal, `DeleteTrashModal`, `ImportModal`, `CreateVaultModal`, and `ChangeMasterPasswordModal`) to strictly match the clean, discrete monochrome geometry of `DevicePairingWizard` (P2P window) and `Onboarding` (first-run setup window).
  - Enforced `rounded-[3px]` geometry, `border border-[var(--border)]`, `bg-[var(--bg-elevated)]` body, and `bg-[var(--bg-surface)]` header styling uniformly across all modals.
  - Standardized modal headers with discrete `h-7 w-7` icon badge containers (`rounded-[3px] border border-[var(--border)] bg-[var(--bg-base)] text-[var(--text-secondary)]`) and clean monochrome close/cancel/confirm action buttons.
  - Eliminated all colorful badges, saturated buttons, and harsh borders (emerald, green, amber, red, purple, blue) across `AttachmentPreviewModal`, `SmartLoginModal`, `TOTPDisplay`, `ImportModal`, `Login` emergency recovery, `VaultSelect` warning and badges, and `SecurityDashboard` StatCard summary cards.
  - Replaced hardcoded color tokens with semantic dark/light theme variables (`var(--border)`, `var(--bg-base)`, `var(--bg-elevated)`, `var(--text-secondary)`, `var(--text-primary)`, `var(--destructive)`).
- **Structured Rekey Wizard in Change Master Password**:
  - Implemented a 2-step verification and configuration flow in `ChangeMasterPasswordModal` (Step 0: current credentials verification; Step 1: new credentials, strength score, and keyfile options).
  - Integrated a discrete 5-segment monochrome password strength meter using semantic theme tokens (`var(--text-primary)`, `var(--border-subtle)`), eliminating saturated multi-colored bars.

### Security
- **Empty (0-Byte) Key File Rejection**:
  - Enforced strict non-zero validation in `read_key_file_safely` (`crates/core/src/vault/manager.rs`), preventing empty files from being accepted as keyfiles which would otherwise silently degrade to password-only derivation.
- **Git Credential Helper Phishing Hardening & Strict Domain Boundary Isolation**:
  - Implemented strict host and subdomain boundary matching (`host == domain || host.ends_with(&format!(".{domain}"))`), preventing cross-domain substring phishing attacks (e.g. `evilgithub.com` or `github.com.attacker.com` matching `github.com`).
  - Hardened `normalize_host` and `extract_domain_stem` against URLs containing user credentials (`user@`, `user:token@`), custom ports, paths, and query fragments, ensuring correct host extraction.
  - Restricted title-based heuristic matching to entries without explicit URLs, preventing domain mismatch bypasses.
- **Unix Socket Path Hardening & IPC Inactivity Auto-Lock Parity**:
  - Enforced strict `0700` permissions (`0o700`) on fallback Unix socket directories under `/tmp` when `$XDG_RUNTIME_DIR` and `$HOME` are unset.
  - Aligned background inactivity timeout (15-minute idle) with explicit lock: `mgr.lock()` purges sensitive vault settings, search tokens, and keys from RAM, `clear_clipboard()` purges system clipboard, the Unix socket file is unlinked, and session tokens are invalidated.
- **Secure File Permissions for Plaintext Exports (CSV / JSON)**:
  - Added `write_sensitive_file_safely` creating export files with atomic `0o600` permissions on Unix and reapplying `set_permissions(0o600)` to ensure sensitive decrypted exports cannot be read by other local OS users.
  - Maintained formula injection sanitization (CWE-1236) across CSV cell values.
- **Atomic Linux Wrap-Key Acquisition & Complete TOCTOU Elimination**:
  - Eliminated the `exists()` TOCTOU race condition in `linux_get_or_create_wrap_key`. Key files are read directly, or created atomically using `OpenOptions::create_new(true).mode(0o600)`, safely handling concurrent initializations without file replacement windows.
- **Blinded Audit Key Zeroization & Zero-Allocation Strength Analysis**:
  - Directly initialized ephemeral 32-byte BLAKE3 blinding keys into `zeroize::Zeroizing::new([0u8; 32])` with `fill_bytes(&mut *ephemeral_key)`, leaving zero unzeroized plaintext key copies on the stack.
  - Replaced intermediate `Vec<u8>` plaintext password heap allocations with zero-allocation borrowed string slices (`std::str::from_utf8(&pwd_bytes)`) for real-time strength analysis.
- **Smart Login TOTP CDP Injection Escaping**:
  - Serialized one-time password characters via `serde_json::to_string` before dispatching to Chromium CDP JavaScript evaluation, preventing string breakout and script injection.

### Changed
- **Change Master Password Modal Geometry & Vertical Compaction**:
  - Eliminated artificial `min-h-[300px]` and `justify-between` on the modal body container in `ChangeMasterPasswordModal`, reducing vertical footprint by ~45% and eliminating empty void spacing.
  - Standardized modal width to `max-w-[420px]` to match `CreateVaultModal` and `Onboarding`.
  - Added structured sub-header divider (`border-b border-[var(--border-subtle)] bg-[var(--bg-base)]/40`) under the stepper progress bar for clean separation from form fields.
- **Discrete Monochrome Theme Harmony**:
  - Replaced colorful badges, saturated buttons, and harsh red/rose/amber styling across `BackupTab`, `TrashTab`, `SecurityTab`, `KeybindsTab`, `DeleteTrashModal`, and `ImportModal` with consistent dark-mode tailored tokens (`var(--border)`, `var(--bg-elevated)`, `var(--text-secondary)`).
- **Git Credential Helper Exact Domain Precedence**:
  - Prioritized exact domain matches (`160` points) over subdomain matches (`140` points) in `score_entry_match`, ensuring specific credentials (e.g. `gist.github.com`) win deterministically over generic parent domain credentials.
- **Bloom Filter Test Isolation**:
  - Converted `test_generate_and_populate_bloom_filter` to in-memory bit validation, preventing test runs from modifying or dirtying `crates/core/data/bloom.bin` on disk.
- **Idiomatic Rust & Clippy Zero-Warning Hygiene**:
  - Cleaned up manual implementations of `div_ceil`, unnecessary mutable bindings, redundant reference dereferences, and collapsible conditionals across all 4 workspace crates (`yntra-crypto`, `yntra-vault-core`, `yntra-cli`, `src-tauri`), compiling cleanly under `-D warnings`.
- **Smooth SPA Navigation for Setup Wizard Rerun**:
  - Replaced hard `window.location.href` assignment with React Router `navigate('/setup')` in `GeneralTab`, avoiding uncoordinated hash reloads.
- **Storage Footprint Metric Localization**:
  - Removed hardcoded unit suffix (`" st"`) in `TrashTab` storage footprint display in favor of clean universal count notation.

### Fixed
- **Mobile Touch Scrolling & Momentum Gestures**:
  - Resolved missing, clipped, and unresponsive touch scrolling across mobile and Android release builds, specifically fixing the entry list in `PasswordList`.
  - Fixed parent flex container height collapse in `AppLayout.tsx` on mobile where `PasswordList`'s `h-full` was treated as indefinite and clipped by `overflow: hidden`.
  - Removed synthetic `onTouchStart`, `onTouchMove`, and `onTouchEnd` event hijacking on `ListItem`, allowing the Android WebView touch compositor thread to immediately recognise vertical kinetic scroll gestures without dead zones.
  - Added `shrink-0` to list sections and items, preventing flexbox compression from collapsing the scrollable boundary.
  - Replaced global `* { touch-action: manipulation; }` with targeted interactive element selectors, and added `.overflow-y-auto * { touch-action: pan-y; }` to guarantee buttons inside lists never block kinetic scrolling.
  - Removed `user-select: none` (`select-none`) from scrollable root containers and modal backdrops, preventing Chromium WebView from misinterpreting touch drags as text selection intents.
  - Added `-webkit-overflow-scrolling: touch`, `overscroll-behavior: contain`, and `touch-action: pan-y / pan-x` across all scrollable views (`PasswordList`, `PasswordDetail`, `SettingsPanel`, `Login`, `VaultSelect`, `Onboarding`).
  - Added flex-column `min-h-0` constraints and eliminated double-nested scroll collisions on mobile detail views.
  - Standardized all 13 application modals with responsive overlay scrolling, viewport height clamping (`max-h-[calc(100dvh-1.5rem)]`), and safe-area insets to prevent clipping on mobile screens and when virtual keyboards open.
  - Decoupled `MobileBottomSheet` drag gestures using `useDragControls` activated exclusively from the drag handle, allowing sheet inner content to scroll freely without accidental sheet dismissals.
- **Mobile & Desktop Launcher Icon Ergonomics & Android Adaptive Sizing**:
  - Fixed an issue where the application icon appeared massively oversized and clipped on Android home screens and desktop shortcuts.
  - Regenerated all Android adaptive icon foregrounds (`ic_launcher_foreground.png` across all mipmap densities: mdpi, hdpi, xhdpi, xxhdpi, xxxhdpi) with 100% transparent backgrounds and balanced ~51% safe-zone scaling (inner 54dp within 108dp canvas), eliminating squircle/circle mask edge clipping.
  - Regenerated desktop and legacy icons (`icon.ico`, `icon.png`, `Square...Logo.png`) with a refined white squircle (~22% corner radius), balanced ~56% logo scaling, and transparent outer padding, delivering brand consistency across Android, Windows, macOS, and Linux.
  - Corrected legacy Android `mipmap-hdpi` icon resolution from non-standard 49x49 to standard 72x72 px.
- **Universal Navigation Translation Keys & Raw Key String Display**:
  - Resolved an issue where buttons in `DevicePairingWizard` and `ChangeMasterPasswordModal` rendered raw translation key strings (`"common.back"`, `"common.next"`) instead of localized text.
  - Added `'common.back'`, `'common.next'`, `'common.hide'`, `'common.show'`, and `'pairing.sync_again'` to `en.ts`, `sv.ts`, and propagated definitions across all 24 supported language dictionaries.
- **Web URL vs Desktop Application Classification**:
  - Fixed classifier heuristics in `smartlogin/classifier.rs` and frontend utility functions where domains with trailing paths (e.g. `https://store.steampowered.com/`) were erroneously flagged as desktop application paths rather than web targets.
- **Login Screen App Logo Rendering**:
  - Fixed logo asset path resolution and contrast on the Login screen, ensuring the Yntra Vault brand mark displays properly across both dark and light themes.
- **Password Visibility ActionTooltips**:
  - Wrapped secret field visibility toggles in `ChangeMasterPasswordModal` with localized `ActionTooltip` components (`login.hide_password` / `login.show_password`).
- **Modal Exit Transitions & Framer Motion Unmount Glitches**:
  - Eliminated premature unmount returns prior to `<AnimatePresence>` across `DeleteEntryModal`, `DeleteTagModal`, `EditTagModal`, `AppPickerModal`, `DeleteTrashModal`, `DevicePairingWizard`, `ImportModal`, and `BackupTab` manual IP prompt, restoring smooth exit animations across all dialogs.
- **Internationalization & Localization Completeness**:
  - Localized hardware lockout countdowns, master password validation errors, device names, and shortcut conflict toasts across `Login`, `BackupTab`, `CreateVaultModal`, `ChangeMasterPasswordModal`, and `KeybindsTab`.
- **Login Key File Translation Key Lookup**:
  - Corrected dictionary key lookups for `login.use_key_file` and `login.key_file_path` in `Login.tsx`, preventing fallback to hardcoded English strings in localized environments.
- **Hardware 2FA Modal Exit Animation Transition**:
  - Restructured JSX hierarchy in `Hardware2FaModal.tsx` to mount conditionally within top-level `<AnimatePresence>`, restoring smooth exit transitions and eliminating early return before animations.
- **WebDAV Cloud Sync Toast Localization**:
  - Replaced hardcoded English sync merge string with localized `settings.sync_success_count`.
- **Emergency Kit Zero-Unwrap Crash Guard**:
  - Replaced `.unwrap()` in `generate_emergency_kit` with `self.keys.as_ref().ok_or(VaultError::VaultLocked)?`, eliminating application abort crashes if invoked when the vault is locked.
- **Windows Explorer File Revealing & Path Validation**:
  - Added path existence checks in `show_in_explorer` and combined `/select,` into a single argument (`/select,<path>`).
  - Added differentiation between files (selected in directory) and directories (opened directly), preventing Explorer from malfunctioning on folder paths.
- **Bloom Filter Byte Conversion Zero-Unwrap**:
  - Replaced `.unwrap()` calls on SHA-256 byte slice conversions in `is_breach_suspected` and bloom tests with unwrap-free array parsing (`unwrap_or([0u8; 8])`), adhering to project zero-panic release invariants.
- **Frontend React Hook Dependencies & Strict Error Typing**:
  - Corrected callback dependencies in `src/pages/Login.tsx` and effect dependencies in `src/pages/VaultSelect.tsx`, typing caught errors to `unknown` for robust production bundle stability.

---

## [0.1.8] - 2026-09-17

### Added
- **Dedicated Pairing Port (Port 5324) & Port Collision Separation**:
  - Separated zero-knowledge device pairing onto dedicated port `5324` (`DEFAULT_PAIRING_PORT`), eliminating port collisions with continuous background Wi-Fi synchronization on port `5322`.
  - Added resilient fallback port traversal (`5324` -> `5325` -> `5322`) during device pairing connections.
  - Added automatic pause and resumption of the background Wi-Fi sync listener during device pairing sessions to prevent socket contention.
- **Collision-Safe Adopt Flow on Unauthenticated Clients**:
  - Implemented safe vault adoption (`ClientPairingMode::AdoptIntoDir`) when pairing from an unauthenticated client instance (`VaultSelect`), saving remote salt and entries into isolated non-colliding files without touching existing local vaults.
- **Active UDP Query-Response P2P Discovery Protocol (`YQRY` / `YPAR`)**:
  - Implemented active bidirectional query-response discovery for instant LAN peering (<50ms). Clients actively pulse `YQRY` queries; hosts respond with direct unicast `YPAR` beacons to the client's address, bypassing router broadcast suppressions and AP isolation.
- **Multi-Interface Local Network Enumeration & Subnet Directed Broadcast**:
  - Added `get_local_lan_ips` probing all active network adapters (Ethernet, Wi-Fi, virtual adapters) via host name resolution and gateway route tests. Discovery packets are broadcast simultaneously across `255.255.255.255`, RFC 2365 administratively scoped multicast (`239.255.53.23`), and directed subnet broadcasts (`x.y.z.255:5323`) for every active interface.
- **Timing-Safe Constant-Time Network Beacon Verification**:
  - Hardened discovery beacon and query verification with `subtle::ConstantTimeEq` across all UDP listener endpoints.
- **Fast-Timeout Candidate Port Probing with Pre-UDP Ping**:
  - Optimized TCP connection establishment in pairing and P2P sync clients with parallel fallback ports `[5324, 5322, 5325]` and a 350ms connect timeout, preceded by direct UDP reachability verification.
- **Strict Trusted Device Deduplication**:
  - Enforced dual-tier deduplication by UUID and case-insensitive `(name, os)` pairs during device registration, pairing, and CRDT synchronization.
- **Pairing UI & Host IP Multi-Interface Ergonomics**:
  - Added one-click IP copy buttons, alternate interface IP selector badges, one-click autofill for the last paired peer IP, and a seamless in-wizard re-synchronization action.
- **Immediate Host Pairing Cancellation (`cancel_pairing_host`)**:
  - Added atomic cancellation support (`pairing_cancel: Arc<AtomicBool>`) and exposed Tauri command `cancel_pairing_host`, immediately unbinding TCP port 5324 and UDP discovery sockets within 40ms when the user cancels or closes the pairing wizard.
- **Real-Time IPv4 Auto-Dot & Local Hostname Formatter**:
  - Implemented automatic octet dot insertion, boundary clamping, local hostname support (`localhost`, `*.local`), and port sanitization (`formatIpv4Input`) with unit test coverage.
- **Host Network Adapter Enumeration Caching**:
  - Implemented adapter IP caching in `broadcast_pairing_beacon_with_ips`, throttling adapter enumeration to every 10 seconds and eliminating UDP socket churn during pairing.
- **Repeat Adoption Collision Guard**:
  - Preserved newly adopted vault paths in `adoptedVaultPathRef` during active pairing wizard sessions, preventing duplicate `<Vault> (1).vdb` creation on subsequent in-wizard syncs ("Sync Again").
- **Comprehensive Wi-Fi & Auto-Sync Notifications**:
  - Added in-app toasts and native desktop notifications (`sendDesktopNotification` via `@tauri-apps/plugin-notification`) for both client discovery auto-sync and incoming peer connections as host, informing users when sync completes, passwords update, or vaults are already in sync.
- **Minimalist Monochrome Pairing Wizard & Stepper Redesign**:
  - Redesigned the Device Pairing Wizard (`DevicePairingWizard.tsx`) to match the exact aesthetic of the first-time setup window (`Onboarding.tsx`), utilizing segmented horizontal progress bars (`h-1 rounded-full`), clean `rounded-[3px]` elevated borders, and a minimal 420px width.
  - Replaced colored accents and green status elements with consistent monochrome palette variables (`var(--text-primary)`, `var(--border)`, `var(--bg-elevated)`).
  - Standardized English defaults across in-code strings and added complete localization keys in `en.ts` and `sv.ts` for pairing steps, role descriptions, hints, and sync notices.

### Changed
- **Progressive Multi-Step Pairing Wizard**:
  - Redesigned `DevicePairingWizard.tsx` into a calm, focused multi-step workflow with visual stepper progress breadcrumbs (`1. Password` ➔ `2. Pairing PIN` ➔ `3. Synchronize`), separating master password verification from 6-digit PIN entry and eliminating visual clutter.
  - Added auto-focus and Enter key progression on the password step, and auto-focus with clipboard paste distribution on the 6-digit PIN inputs.
  - Displayed resolved host pairing IP and dedicated port (`<ip>:5324`) on the host screen.

### Security
- **Unauthenticated Client Adopt Mode Isolation**:
  - Enforced strict session authentication checks in `start_pairing_client`: clients pairing from the `VaultSelect` screen or locked states operate exclusively in `ClientPairingMode::AdoptIntoDir`.
  - Prohibited unauthenticated background decryption or transmission of local `.vdb` files on disk when pairing from logged-out states.
  - Adopted vaults are written to dedicated, collision-resistant filenames derived from the host vault's metadata name (`<HostVaultName>.vdb`, `<HostVaultName> (1).vdb`), completely eliminating silent overwriting of unrelated local vaults.
- **P2P Discovery In-Loop Self-Echo Filtering**:
  - Relocated local LAN IP and loopback filtering directly inside the packet receive loop of `listen_discovery_beacon`, ensuring discovery listeners do not short-circuit on their own UDP broadcast reflections and allowing remote LAN peers to be discovered reliably.
- **IPv4 LAN Prioritization & IPv6 Link-Local Isolation**:
  - Excluded un-routable IPv6 link-local (`fe80::/10`) and multicast addresses from `is_valid_lan_ip`, ensuring `get_local_lan_ips()` and `get_local_lan_ip()` prioritize valid, reachable IPv4 addresses on multi-homed interfaces.
- **Windows DOS Reserved Filename Sanitization**:
  - Hardened `sanitize_vault_filename` against reserved Windows device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) during client adopt mode database initialization.

### Fixed
- **P2P LAN Discovery Blind Spot (Self-Echo)**:
  - Fixed a critical issue where `listen_discovery_beacon` received its own beacon and terminated the discovery process before remote devices could reply.
- **Host Pairing TCP/UDP Port Hang**:
  - Fixed an issue where closing or cancelling the pairing wizard left TCP port 5324 and UDP port 5323 bound for up to 180 seconds, blocking subsequent pairing attempts.
- **IPv6 Link-Local Address Presentation**:
  - Fixed an issue on Windows where link-local `fe80::` addresses were displayed as the host's primary pairing IP, causing connection failures when entered into clients.
- **Duplicate Vault Accumulation on Re-Sync**:
  - Fixed an issue where clicking "Sync Again" after adopting a vault created duplicate incremental database files on disk (`<Name> (1).vdb`).

---

## [0.1.7] - 2026-09-15

### Added
- **Independent Multi-Platform Release Distribution**:
  - Automated CI release packaging for Linux (`.AppImage`, `.deb`), Android (signed universal `.apk`), and Windows (`.exe` setup, `.msi`, portable `.exe`, standalone CLI).
  - Implemented automated Android release signing via `jarsigner` with release keystore generation.
- **Standalone Windows Portable & CLI Executables**:
  - Published self-contained single-executable portable desktop bundle (`Yntra.Vault_0.1.7_portable.exe`) running without installation or administrator rights.
  - Published standalone command-line interface executable (`Yntra.Vault_0.1.7_cli.exe`) for terminal workflows and automated scripting.
- **Unified Trusted Devices Architecture**:
  - Implemented persistent device pairing registry in `VaultSettings.trusted_devices` with device UUID, human-readable name, OS badge, hardware type, and sync timestamps.
  - Added mutual device metadata exchange during 6-digit PIN handshake (`DeviceInfo`, `resolve_local_device_info`).
  - Added host-side device session revocation (`revoke_trusted_device`), actively expelling unapproved devices from the encrypted `.vdb` settings payload.
  - Added cryptographic revocation rejection signal (`P2P_REVOKED_SIG`), immediately closing connections from revoked devices and prompting the client to re-pair.
  - Added continuous, code-free background auto-synchronization for paired devices on local Wi-Fi.
- **Zero-Knowledge UDP Discovery Beacon (Port 5323)**:
  - Added zero-knowledge discovery beacon exchange via BLAKE3 keyed hash tokens (`compute_pairing_beacon_id`, `compute_p2p_discovery_id`) for automatic LAN discovery without manual IP entry.
- **Zero-Vault Adopt Flow on New Clients**:
  - Added automatic standard vault file initialization (`Documents/YntraVault/yntra-vault.vdb`) when pairing from an uninitialized client instance, adopting remote salt and entries seamlessly.
- **Trusted Devices Management UI**:
  - Added trusted devices section in Settings > Backup (`BackupTab.tsx`) with device type icons, OS labels, pairing dates, last sync timestamps, and per-device disconnect ("Koppla från") action.
  - Added single unified toggle for background Wi-Fi synchronization.

### Changed
- **6-Digit PIN Pairing Wizard UX**:
  - Upgraded PIN input in `DevicePairingWizard.tsx` to 3+3 triplet layout with clipboard paste (Ctrl+V) distribution and Enter key submission.
  - Replaced native browser checkboxes with standardized `<Toggle />` components across pairing and backup settings.
  - Refined action button margins and text container padding to eliminate viewport clipping.

### Security
- **P2P Transit AEAD Encryption (Defense-in-Depth)**:
  - Encrypted all database network payloads over TCP with XChaCha20-Poly1305 AEAD (`P2P_TRANSIT_AAD`) using `subkeys.vault_key`, maintaining transparent backward compatibility for unencrypted legacy payloads.
- **Zero-Knowledge Salt Commitment (P2P Handshake)**:
  - Replaced plaintext Argon2id root salt transmission over TCP with a 32-byte keyed BLAKE3 commitment (`compute_salt_commitment`), verifying vault compatibility early without disclosing the root salt.
- **Argon2id-Hardened Pairing Discovery Beacon**:
  - Replaced fast un-salted BLAKE3 PIN hashing with Argon2id-derived pairing subkeys (`compute_pairing_beacon_id_from_subkeys`, `compute_pairing_beacon_id`), preventing LAN eavesdroppers from conducting offline dictionary attacks against 6-digit PINs.
- **Encrypted Host Root Salt in Pairing**:
  - Encrypted the host pairing response payload (`PairingHostPayload`) using XChaCha20-Poly1305 AEAD (`PAIRING_AAD_HOST`) keyed with pairing subkeys, eliminating plaintext Argon2id salt transmission during initial device pairing.
- **Device Revocation & Nil-UUID Rejection**:
  - Enforced strict revocation checks rejecting `Uuid::nil()` or unlisted client devices with `P2P_REVOKED_SIG` when trusted devices are configured. Bound `client_device_uuid` directly into the client's mutual HMAC signature.
- **Volatile Memory Zeroization in P2P & Pairing**:
  - Wrapped all decrypted and merged database buffers in `zeroize::Zeroizing<Vec<u8>>` across `crates/core/src/services/sync/mod.rs` and `crates/core/src/services/sync/pairing.rs`.
- **Frontend Ephemeral Password Zeroing**:
  - Added unconditional state zeroization (`setPassword('')`, `setInputDigits(...)`) on modal close, completion, and unmount in `DevicePairingWizard.tsx`.
- **LAN Discovery Self-Echo Suppression**:
  - Added loopback and local LAN IP filtering in `scan_p2p_discovery` (`src-tauri/src/commands/sync.rs`), preventing desktop instances from discovering and attempting to sync with their own listeners.
- **P2P Handshake Timing and Asymmetry Hardening**:
  - Replaced dynamic length rejection responses with uniform 64-byte `P2P_AUTH_FAILED_SIG` and constant-time verification (`subtle::ConstantTimeEq`), preventing client panics and timing side-channels.
- **Root Salt Mismatch Enforcement**:
  - Enforced mutual root salt checks (`P2P_SALT_MISMATCH_MARKER`) early in the handshake phase before decrypting or processing remote payloads.
- **Constant-Time Hardware 2FA Comparison**:
  - Replaced non-constant-time byte comparison in `enable_hardware2fa_with_password()` with `subtle::ConstantTimeEq` (`ct_eq`), closing a timing side-channel during Hardware 2FA enrollment.
- **Session Token Path & Storage Hardening**:
  - Unified Windows session token location between CLI (`yntra-cli`) and core (`yntra-crypto`) to `%LOCALAPPDATA%/Yntra Vault/session.token`.
  - Removed plaintext fallback to ensure session tokens are strictly encrypted via App-Bound DPAPI / BLAKE3.
- **In-Memory Transit Secret Scrubbing**:
  - Wired `clearSessionSecrets()` to the `vault-connection-lost` event in `AuthContext`, ensuring transit import and sync credentials in volatile memory are immediately wiped on disconnect.
- **Key File POSIX Permission Hardening**:
  - Enforced POSIX file mode `0o600` (`read/write` owner-only) and filesystem sync (`sync_all()`) on newly generated key files in `rekey.rs`.
- **KDF Lower Bound Alignment**:
  - Normalized Argon2id lower resource bounds to 64 MB (`65_536 KB`) and 2 iterations across core validation logic, architecture specifications, and agent guidelines.

### Fixed
- **Linux Key Wrapping Enum Variant**:
  - Corrected wrap key failure mapping in `linux_get_or_create_wrap_key` (`crates/crypto/src/tpm.rs`), returning `VaultError::TpmError` instead of invalid enum variants.
- **32-Bit Linux & Android Memory Limit Arithmetic**:
  - Fixed integer overflow and type mismatch in `crates/crypto/src/mem.rs` by casting allocations to `libc::rlim_t` with saturating addition, ensuring compatibility with 32-bit Android architectures (`armv7-linux-androideabi`, `i686-linux-android`).
- **Cross-Platform Window Handle Gating**:
  - Gated Win32-specific window handle retrieval (`window.hwnd()`) behind `#[cfg(target_os = "windows")]` in `src-tauri/src/lib.rs`, `src-tauri/src/commands/tools.rs`, and `src-tauri/src/commands/auth.rs`, with safe non-Windows fallbacks.
- **Desktop-Only Window Configuration Gating**:
  - Gated `window.set_always_on_top()` behind `#[cfg(desktop)]` in `src-tauri/src/commands/auth.rs` to ensure clean compilation on Android and mobile targets.
- **Host Device Authorization Fail-Closed Enforcement**:
  - Replaced silent error absorption (`if let Ok(...)`) during local vault read/decryption in P2P handshake with strict error propagation (`SyncError`), ensuring connections fail-closed if the trusted devices registry cannot be verified.
- **Authoritative Trusted Devices Re-Sync**:
  - Adopted host's authoritative `trusted_devices` list on the client during return synchronization, automatically propagating newly paired or revoked devices across all client vaults.
- **P2P Socket Interface Binding**:
  - Switched default sync listener binding from loopback (`127.0.0.1`) to all interfaces (`0.0.0.0:5322`), enabling cross-device mobile connections on local LANs.
- **Listener and Client Connection Hangs**:
  - Implemented non-blocking socket handling with bounded timeouts in `run_p2p_sync_listener` to prevent indefinite background thread lockups.
  - Replaced unbounded `TcpStream::connect` with `connect_timeout` (2s) in `run_p2p_sync_client`.
- **Sync Tag and State Desynchronization**:
  - Synchronized `refreshTags()` alongside `refreshEntries()` upon all P2P and WebDAV merge operations.
- **macOS Native Clipboard Argument Passing**:
  - Replaced broken `cat /dev/stdin` AppleScript pipe with `on run argv` script arguments in `crates/crypto/src/clipboard.rs`, resolving empty string returns during clipboard operations on macOS.
- **Mobile Safe Area & Notch Layout**:
  - Added `viewport-fit=cover`, `maximum-scale=1.0`, and `user-scalable=no` to the viewport meta tag in `index.html` for proper edge-to-edge rendering and zoom prevention.
  - Separated top safe-area padding (`env(safe-area-inset-top)`) from header height in the mobile detail view (`AppLayout.tsx`), preventing squashed headers on devices with Dynamic Island or notch.
- **Mobile Bottom Navigation & Scroll Margins**:
  - Dynamically sized the bottom navigation bar to `calc(4rem + env(safe-area-inset-bottom))` in `MobileBottomNav.tsx` to prevent icons and labels from compressing against the home bar.
  - Added bottom padding offsets to entry list and detail view scroll containers to prevent content from being occluded beneath the fixed navigation bar.
- **Mobile Drawer & Bottom Sheet Exit Animations**:
  - Moved conditional rendering inside `<AnimatePresence>` in `MobileDrawer.tsx` and `MobileBottomSheet.tsx`, enabling smooth Framer Motion exit transitions on close.
- **Mobile Onboarding Overflow**:
  - Replaced fixed `w-[420px]` container with `w-full max-w-[420px]` and `min-h-dvh overflow-y-auto` in `Onboarding.tsx`, eliminating horizontal clipping on narrow mobile viewports.
- **Mobile Localization Parity**:
  - Extracted hardcoded English strings in mobile views to translation keys (`mobile.back_to_vault`, `mobile.switch_vault`, `mobile.zero_knowledge_vault`) with Swedish and English definitions.

### Removed
- **Legacy Dual Sync Controls**:
  - Removed obsolete standalone "Sync Now via Wi-Fi" buttons and manual network listener toggles from Settings, unifying all local synchronization under Trusted Devices.

---

## [0.1.6] - 2026-09-15

### Added
- **Cryptographic Emergency Kit**:
  - Implemented 2-of-3 Shamir Secret Sharing recovery sheet generator with direct `.md` export.
  - Added encrypted audit logging for Emergency Kit generations, resets, and rekey invalidations.
- **Steam Guard TOTP Algorithm**:
  - Added dynamic HMAC-SHA1 alphanumeric 5-character token generation for Steam Guard accounts.
  - Added automatic detection for `steam://` URIs and `issuer=Steam`.
- **Storage Compaction & Trash Lifecycle**:
  - Added background storage compaction engine (`compact_vault`) and automatic 30-day trash purging (`purge_expired_trash`).
  - Added real-time storage metrics breaking down byte usage across active vs trashed items and attachments.
- **First-Run Onboarding Setup**:
  - Added setup wizard with Standard vs Closed System (air-gapped) privacy modes.
- **Security Dashboard Improvements**:
  - Clustered reciprocal reused password detections with direct per-entry remediation navigation.
  - Unified TOTP code view and 2FA recovery backup codes into a cohesive card.
- **Full Internationalization Parity (24 Languages)**:
  - Achieved complete key parity across all 24 supported locales (839 keys per language).

### Security
- **Smart Login ccTLD Base Domain Isolation**:
  - Enforced effective TLD+1 (`eTLD+1`) boundary matching with multi-part ccTLD parsing (`.co.uk`, `.com.au`), preventing credential leakage across public suffixes.
  - Restricted automated credential navigation to exact domain, same base domain, or explicit SSO pairings in `AUTH_DOMAINS`.
- **Smart Login CDP String Injection Prevention**:
  - Serialized all DOM-derived text targets using `serde_json::to_string` before JavaScript evaluation in Chrome DevTools Protocol.
- **Emergency Kit Keyed HMAC Fingerprinting**:
  - Replaced raw unkeyed hashes with keyed HMAC-SHA512 verification checksums derived from active session subkeys.
- **CSV Formula Injection Sanitization (CWE-1236)**:
  - Prepended escape single quotes to cell values starting with formula triggers (`=`, `+`, `-`, `@`, `\t`, `\r`) during CSV export.
- **Settings & Transit Memory Scrubbing on Vault Lock**:
  - Actively cleared WebDAV credentials, remote endpoints, and emergency audit logs from memory on lock.
  - Wrapped transient AEAD subkey vectors in `zeroize::Zeroizing` before transferring into `LockedBuffer`.
- **Hardware 2FA Argon2id Key Stretching**:
  - Enforced 256MB RAM Argon2id key derivation for Hardware 2FA envelope Key Encryption Keys.
- **Keyfile Factor Isolation**:
  - Removed plaintext keyfile path persistence from client browser storage.
- **Least Privilege Desktop Capabilities**:
  - Revoked clipboard-read permission and stripped development tools from production release capabilities.
- **Timing Attack Resistance**:
  - Enforced constant-time equality comparisons (`subtle::ConstantTimeEq`) across TOTP verification and local CLI daemon authentication.

### Fixed
- **Release-Mode Startup Crash Prevention**:
  - Replaced unhandled system tray window icon unwrap with safe pattern matching, preventing process aborts under `panic = "abort"`.
- **Tauri v2 Production CSP Dual-Scheme Parity**:
  - Added `https://ipc.localhost` to Content Security Policy `connect-src`, eliminating silent IPC invoke failures in Windows WebView2 release packages.
- **React Hook Lifecycle Ordering & Minified Errors**:
  - Relocated state hooks to top-level unconditional component scope, eliminating fatal `Minified React error #300`/`#310` crashes in production bundles.
- **Cross-Crate Hardware 2FA Test Gating**:
  - Removed `cfg!(debug_assertions)` wrappers from hardware 2FA and biometric test harnesses to ensure integration tests execute reliably under `cargo test --release`.
- **2FA Recovery Codes Mapping**:
  - Sanitized persistence pipeline so recovery backup codes are stored exclusively in dedicated fields and do not leak into custom fields.
- **KeePass XML Importer**:
  - Added support for protected in-memory values and XML entity decoding.

---

## [0.1.5] - 2026-09-14

### Fixed
- **Desktop SPA Route Resolution & Blank Screen Elimination**:
  - Migrated from `BrowserRouter` to `HashRouter` for desktop SPA protocol compatibility, preventing 404 blank screens when reloading pages in production release builds.
  - Implemented global `ErrorBoundary` UI component to capture unexpected render/layout exceptions, displaying recovery options and technical stack traces rather than leaving a blank screen.
  - Registered window runtime error and unhandled rejection listeners to ensure full diagnostic visibility in webview logs.
- **Framer Motion Reorder & State Stability**:
  - Refactored sidebar tag ordering in `Sidebar.tsx` to track stable primitive string IDs rather than volatile tag object instances, eliminating reference desynchronization and layout projection crashes during password saving and tag mutation.
  - Replaced `Reorder.Group` with standard list rendering when sorted by name or count, avoiding unnecessary animation frame projection overhead.
- **Tag ID Parity on Creation**:
  - Updated `addTag` optimistic update to record the real backend UUID returned from the database, preventing temporary client UUID divergence.
- **Atomic Save & Filesystem Watcher Race Guard**:
  - Added in-flight `.vdb.tmp` atomic save detection and a 200ms debounce in the background filesystem watcher thread to prevent false-positive connection lost events during vault file flushes.
- **Keyboard Text Input Caret Fix**:
  - Fixed an issue where outer container `select-none` styles prevented caret positioning and keyboard input in password fields while still permitting clipboard pasting.
- **Developer Tools & Native Inspection**:
  - Enabled Tauri `devtools` feature flag in release builds and added `Shift + Right Click` bypass to allow inspecting elements directly in production binaries.

---

## [0.1.4] - 2026-09-13

### Fixed
- **Smart Login Release Mode & Administrator Elevation Compatibility**:
  - Implemented automatic process de-elevation via Windows Explorer primary token duplication (`CreateProcessWithTokenW`) when Yntra Vault is executed with Administrator privileges, enabling Chromium-based browsers (Brave, Chrome, Edge) to launch with full sandbox security under the interactive desktop user session.
  - Added fallback compatibility mode with `--no-sandbox`, `--disable-gpu-sandbox`, and `--test-type` for non-shell/headless environments.
  - Corrected Chromium CLI extension flag from malformed `--disable-extensions-except=` to `--disable-extensions`.
  - Isolated standard streams (`Stdio::null()`) to prevent broken console handle inheritance under Windows GUI subsystem (`windows_subsystem = "windows"`).
  - Enforced browser executable parent directory as current working directory to prevent permission errors when launched from `Program Files`.
  - Enhanced stale lock cleanup by removing `SingletonCookie` and `SingletonSocket` alongside `SingletonLock` and `lockfile`.

---

## [0.1.3] - 2026-09-13

### Added
- **Complete Internationalization (24 Languages)**:
  - 100% localized translation coverage across all 24 supported locales: English (`en`), Swedish (`sv`), Danish (`da`), Norwegian (`no`), Finnish (`fi`), German (`de`), Dutch (`nl`), French (`fr`), Spanish (`es`), Italian (`it`), Portuguese (`pt`), Polish (`pl`), Czech (`cs`), Russian (`ru`), Ukrainian (`uk`), Turkish (`tr`), Greek (`el`), Hebrew (`he`), Arabic (`ar`), Hindi (`hi`), Japanese (`ja`), Korean (`ko`), Simplified Chinese (`zh-CN`), and Traditional Chinese (`zh-TW`).
  - Exactly 777 translated keys per language with 0 missing strings, validated by comprehensive i18n test suites.
- **Localized Interactive Tutorial & Onboarding**:
  - All walkthrough steps, tag names, password strength indicators, and credential placeholders in `VaultTutorial.tsx` are now fully translated.

### Changed
- **Package Identity**:
  - Corrected application package identifier in `package.json` from generic `my-app` template to official `yntra-vault`.

### Fixed
- **Translation Key Discrepancies**:
  - Fixed typo in `login.err_enter_password` key reference in `Login.tsx`.
- **Smart Login Browser Visibility**:
  - Removed `CREATE_NO_WINDOW` (`0x08000000`) flag during browser process spawn on Windows to ensure browser windows always open visibly in production release builds.
- **Process & Stale Lockfile Cleanup**:
  - Enhanced browser shutdown polling (3.5-second timeout) and automated purging of orphaned `SingletonLock` / lockfile artifacts in user data directories.

---

## [0.1.2] - 2026-09-13

### Fixed
- **Smart Login Release Process Spawning**:
  - Resolved headless flag and process creation flags causing Chromium instances to terminate silently in packaged release builds.
- **Browser Lockfile Handling**:
  - Added robust detection and removal of stale profile lockfiles before establishing Chrome DevTools Protocol (CDP) WebSocket sessions.

---

## [0.1.1] - 2026-09-13

### Added
- **Multi-Layer Hardware Key Wrapping (Hardware Envelopes)**:
  - Windows TPM 2.0 (`MS_PLATFORM_KEY_STORAGE_PROVIDER`) RSA key wrapping tagged with `b"YTPM"`.
  - Windows App-Bound DPAPI (`CryptProtectData`) with installation-bound BLAKE3 entropy tagged with `b"YDPB"`.
  - Backward compatibility detection for legacy untagged DPAPI blobs.
- **Passkey Authenticator**:
  - Native ECDSA over NIST P-256 (ES256) keypair generation and signing directly per entry.

### Changed
- **4-Crate Workspace Architecture**:
  - Decoupled monolithic core into clean Rust crates:
    - `crates/crypto` (`yntra-crypto`): Pure auditable cryptography, canary guard pages, and volatile zeroization.
    - `crates/core` (`yntra-vault-core`): Storage formats, database engine, manager CRUD, and services.
    - `crates/cli` (`yntra-cli`): High-speed CLI binary (`yntra.exe`) and session daemon.
    - `src-tauri` (`yntra-vault-app`): Desktop GUI wrapper.

---

## [0.1.0] - 2026-09-12

### Added
- **Core Cryptographic Storage Pipeline**:
  - `.vdb` binary vault format (v4) with single-pass XChaCha20-Poly1305 AEAD header binding.
  - Argon2id KDF (256 MB RAM, 4 iterations, 4 threads) + HKDF-SHA512 subkey derivation.
- **TOTP Authenticator**:
  - RFC 6238 compliant two-factor authentication generator (SHA-1, SHA-256, SHA-512).
- **Encrypted Search**:
  - Zero-disclosure trigram HMAC-SHA256 fuzzy search.
- **Autotype Engine**:
  - Windows UI Automation (UIA) input simulation with safety locks and window verification.
- **WebDAV Synchronization**:
  - Item-level 3-way merge with RFC 7232 ETag conditional requests and tombstone preservation.
- **CLI & Terminal TUI**:
  - High-performance command line client and Ratatui terminal user interface.
