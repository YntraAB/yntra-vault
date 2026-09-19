# Yntra Vault — Technical Specification

## Runtime Dependencies

### Frontend

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | ^19.1.0 | UI framework |
| `react-dom` | ^19.1.0 | DOM rendering |
| `react-router-dom` | ^7.6.0 | Client-side routing (vault select → login → app) |
| `framer-motion` | ^12.12.0 | Animations (panel slides, list stagger, modal transitions) |
| `lucide-react` | ^0.511.0 | Icon system |
| `geist` | ^1.4.0 | Geist Sans + Mono font families |
| `clsx` | ^2.1.0 | Conditional classnames |
| `tailwind-merge` | ^3.3.0 | Tailwind class deduplication |

### Backend (Rust Workspace)

| Crate | Purpose |
|-------|---------|
| `argon2` | KDF — Argon2id (memory-hard, GPU-resistant, 256 MB RAM) |
| `chacha20poly1305` | Vault-level AEAD (XChaCha20-Poly1305) with header AAD binding |
| `aes-gcm` | Entry-level AEAD (AES-256-GCM legacy fallback) |
| `hkdf` + `sha2` | Subkey derivation (HKDF-SHA512) |
| `hmac` | P2P handshake authentication & legacy v1/v2 file verification |
| `p256` | Passkey generation and signing (ECDSA P-256 / ES256) |
| `zeroize` | Memory safety — automatic sensitive buffer clearing |
| `chromiumoxide` | Chrome DevTools Protocol (CDP) client for zero-extension Smart Login |
| `rmp-serde` | Vault payload serialization (MessagePack format) |
| `clap` + `clap_complete` | Command line argument parsing & shell completion generator |
| `ratatui` + `crossterm` | Interactive Terminal UI (TUI) rendering & terminal event loop |
| `rpassword` | Terminal interactive password prompt |
| `comfy-table` | Terminal table output formatting |
| `colored` | Terminal ANSI color highlighting |
| `windows-sys` / `winreg` | Windows TPM 2.0, App-Bound DPAPI, and UAC elevation checks |

---

## Component Architecture

### Layout

| Component | Description |
|-----------|-------------|
| `AppLayout` | Three-panel CSS Grid: sidebar + password list + detail |
| `ResizablePanel` | Drag-handle wrapper, widths persisted to localStorage |

### Core Components

| Component | Description |
|-----------|-------------|
| `Sidebar` | Navigation: All, Favorites, Tags, Settings, Security |
| `PasswordList` | Search bar + scrollable entry list with sections |
| `PasswordDetail` | Entry view: fields, TOTP, passkey, recovery codes, password history |
| `EntryModal` | Create/edit entry form with field type selection |
| `SmartLoginModal` | Zero-extension automated login flow: browser picker, CDP status, and progress streaming |
| `VaultTutorial` | Interactive onboarding walkthrough guiding users through core features |
| `SettingsPanel` | Slide-in overlay: General, Appearance, Security, WebDAV Cloud Sync, Backup tabs |
| `SecurityDashboard` | Audit results: weak/reused/old/breached password breakdown |

---

## Internationalization (i18n)

Yntra Vault features a zero-dependency, type-safe internationalization engine (`src/i18n/`):

- **24 Supported Locales**: English (`en`), Swedish (`sv`), Danish (`da`), Norwegian (`no`), Finnish (`fi`), German (`de`), Dutch (`nl`), French (`fr`), Spanish (`es`), Italian (`it`), Portuguese (`pt`), Polish (`pl`), Czech (`cs`), Russian (`ru`), Ukrainian (`uk`), Turkish (`tr`), Greek (`el`), Hebrew (`he`), Arabic (`ar`), Hindi (`hi`), Japanese (`ja`), Korean (`ko`), Simplified Chinese (`zh-CN`), Traditional Chinese (`zh-TW`).
- **Complete Coverage Invariant**: Every locale provides exact 1:1 coverage with `en` (824 translation keys), enforced via `translations.test.ts`.
- **Right-To-Left (RTL)**: Dynamic document directionality (`dir="rtl"`) automatically toggled for Arabic and Hebrew locales.
- **Dynamic Font & Locale Switching**: Instant UI re-render without app reloading via `LanguageContext`.

---

## Synchronization Protocol (WebDAV & P2P)

### WebDAV Cloud Sync
- **Optimistic Concurrency**: Uses HTTP `If-Match` headers with normalized ETags (RFC 7232).
- **Conflict Resolution**: On HTTP 412 (Precondition Failed), executes up to 3 optimistic retry attempts: fetches remote ETag, downloads remote payload, performs item-level 3-way merge with tombstone preservation, saves locally, and retries conditional PUT.
- **Transport Security**: Enforces HTTPS scheme for non-localhost endpoints via strict `url::Url` host validation (`http://` allowed strictly for loopback hosts: `localhost`, `127.0.0.1`, `[::1]`).
- **Memory Protection**: Decrypted payload buffers zeroed using `Zeroize`. Remote credentials held strictly in volatile in-memory registry (`sessionSecrets.ts`), wiped upon vault lock.

### Peer-to-Peer (P2P) Direct Sync & Zero-Knowledge Device Pairing

#### 1. Background Wi-Fi Sync (Port 5322)
- **Continuous LAN Synchronization**: Paired devices automatically synchronize encrypted vault payloads over local Wi-Fi without cloud reliance or manual trigger.
- **Client-First Mutual Authentication**: Handshake enforces strict client-first verification. The connecting client must prove identity via HMAC-SHA512 over `server_challenge` bound to the client's UUID before the server returns any signature or payload, permanently preventing unauthenticated password oracles.
- **End-to-End AEAD Encryption**: Transferred database archives are enveloped with ephemeral XChaCha20-Poly1305 using `P2P_TRANSIT_AAD`.
- **Authoritative Revocation Signal (`P2P_REVOKED_SIG`)**: Connecting devices that have been removed from the host's `trusted_devices` list are rejected immediately with a revocation signal, prompting the client to re-pair.

#### 2. Zero-Knowledge Device Pairing Protocol (Port 5324)
- **Ephemeral Transit Tunnel**: Derives one-time pairing subkeys (`VaultKey`, `HmacKey`) from `MasterPassword` + `6-digit PIN` via BLAKE3 domain separation, Argon2id, and HKDF-SHA512.
- **Port Collision Separation**: Pairing operates on dedicated port `5324` (`DEFAULT_PAIRING_PORT`), avoiding port contention with the continuous background sync listener on port `5322`. The background sync listener is automatically paused during active pairing wizards.
- **Immediate Host Cancellation & Port Release**: Host TCP and UDP listeners poll an atomic cancellation flag (`pairing_cancel: Arc<AtomicBool>`) every 40ms via the command `cancel_pairing_host`. Listening sockets are released immediately upon user cancellation or modal dismissal, preventing port lockups on port 5324.
- **Strict Adopt Mode (`ClientPairingMode::AdoptIntoDir`)**: When pairing from an unauthenticated client state (e.g. `VaultSelect`), the client reads zero disk files, transmits zero entries (`client_entries_count = 0`), and saves the adopted database into a dedicated, collision-free file (`<HostVaultName>.vdb`, `<HostVaultName> (1).vdb`).
- **Windows DOS Device Name Neutralization**: Vault filenames are sanitized against reserved Windows DOS device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) during client adopt mode database initialization.
- **Session-Bound Adopt Path Preservation**: During an active pairing wizard session, the newly adopted vault path is preserved in volatile memory (`adoptedVaultPathRef`) so subsequent in-wizard re-synchronizations update the existing file without spawning duplicate `<Vault> (1).vdb` files.
- **Mutual Device Metadata Exchange**: Devices securely exchange `DeviceInfo` (UUID, device name, hardware type, OS) and register them into `VaultSettings.trusted_devices` with strict deduplication by UUID and `(name, os)`.

#### 3. Optical QR-Code Zero-Knowledge Pairing Protocol (Protocol `YQR2`)
- **Optical Pre-Shared Key & Custom URI**: Host generates a single-use 256-bit CSPRNG optical secret and UUIDv4 session ID with a 90-second TTL. The QR code encodes `yntrapair://v2?id=<session_id>&s=<secret_hex>&ip=<ip>&p=<port>&sas=<sas>&name=<name>`. Master passwords and database payloads are never placed in the optical QR image.
- **Short Authentication String (SAS)**: 4-digit decimal visual confirmation code derived deterministically via BLAKE3 (`b"yntra-qr-sas-v2"`), displayed simultaneously on host display and client camera viewfinder for out-of-band MitM detection.
- **Mutual Handshake (`YQR2`)**: Client transmits 68-byte frame (`[YQR2 | session_id | client_challenge | HMAC]`), verified by host in constant time before host responds with 48-byte signed challenge. Unauthenticated peers are rejected with zero metadata disclosure.
- **Dynamic Session AAD Binding**: Ephemeral transit payloads use `XChaCha20-Poly1305` authenticated with dynamic Additional Authenticated Data (`yntra-qr-transit-v2:<session_id>`), eliminating cross-session replay vulnerabilities.
- **Zero-IPC Plaintext Isolation**: Provisioned master passwords for biometric enrollment are held exclusively in `zeroize::Zeroizing` buffers in native `AppState.pending_adopted_vault`. Plaintext credentials never cross the Tauri IPC bridge into webview JavaScript (eliminating `receivedMasterPassword` in V8).
- **Safe Adoption & Collision Avoidance**: When pairing without provisioned credentials (`include_password: false`), client safely adopts into `<HostVaultName>.vdb` without data loss and prompts user to verify the existing master password via `complete_adopted_vault`.
- **Dual-Engine Camera Scanner (`QrScannerModal.tsx`)**: Hardware-accelerated browser-native `BarcodeDetector` with automatic `jsQR` canvas fallback, front/rear camera switcher, and drag-and-drop image file optical scan.

#### 4. Active UDP Query-Response Discovery (Port 5323)
- **Active Query-Response Protocol**: Clients emit periodic `YQRY` query pulses (`[YQRY (4B) | pairing_beacon_id (32B)]`). Hosts respond with immediate direct unicast `YPAR` packets (`[YPAR (4B) | pairing_beacon_id (32B) | local_port (2B)]`), reducing discovery latency from 7 seconds to <50ms and penetrating AP isolation.
- **In-Loop Self-Echo Isolation**: The receive loop in `listen_discovery_beacon` evaluates received packet source IP addresses against known local network adapters and loopback *inside* the receive loop. Self-echo packets are dropped immediately, allowing the listener to continue until remote peers respond or the timeout expires.
- **Multi-Interface Local Network Enumeration & IPv4 Prioritization**: `get_local_lan_ips` probes all active network adapters (Ethernet, Wi-Fi, mobile hotspots, virtual adapters), excludes un-routable link-local IPv6 addresses (`fe80::/10`) and multicast, and sorts IPv4 addresses first to guarantee reachable network endpoints in the UI.
- **Adapter Enumeration Caching**: Local network interface addresses are cached during the beacon loop (`broadcast_pairing_beacon_with_ips`) and refreshed at most every 10 seconds, eliminating redundant socket allocations and system calls.
- **Subnet Directed Broadcast & Multicast**: Discovery beacons are broadcast simultaneously to `255.255.255.255`, RFC 2365 administratively scoped multicast (`239.255.53.23`), and directed subnet broadcasts (`x.y.z.255:5323`) across all detected interfaces.
- **Timing-Safe Constant-Time Verification**: All UDP token comparisons enforce constant-time equality via `subtle::ConstantTimeEq`.
- **Parallel Candidate Port Fallback**: Clients test candidate ports `[confirmed_host_port, 5324, 5322, 5325]` with a snappy 350ms LAN timeout and direct UDP pre-ping.
- **Real-Time Auto-Dot & Hostname Formatting**: `formatIpv4Input` validates and formats IP addresses with automatic octet dot insertion, supports local hostnames (`localhost`, `*.local`), and sanitizes port segments.

#### 5. Unified Pairing Wizard & Sync Notifications
- **Segmented Mode Switcher (`DevicePairingWizard.tsx`)**: Tabbed interface offering instant `QR-kod (Snabbast)` as optical default alongside `6-siffrig PIN` as manual fallback.
- **Monochrome Minimalist Aesthetic**: Uses clean design system tokens (`var(--text-primary)`, `var(--border)`, `var(--bg-elevated)`), eliminating colored badges and green success elements. Matches the exact aesthetic of the first-time setup window (`Onboarding.tsx`) with segmented horizontal progress bars (`h-1 rounded-full`).
- **Comprehensive Desktop & In-App Sync Notifications**: Dispatches native desktop notifications (`sendDesktopNotification` via `@tauri-apps/plugin-notification`) and in-app toasts for both host listener sync and client auto-discovery sync, notifying users of synced credential counts or confirming up-to-date status.
- **English In-Code Defaults & Full Localization**: All wizard and notification strings default to English in source code with complete localization keys in `en.ts` and `sv.ts`.

### Shared Components

| Component | Used By |
|-----------|---------|
| `CopyButton` | All fields — copy with checkmark animation |
| `AutotypeButton` | All fields — triggers OS-level input |
| `PasswordInput` | Login, entry edit — show/hide toggle |
| `PasswordStrength` | Entry detail, generator — visual strength bar |
| `BreachIndicator` | Entry list, detail — breach status badge |
| `Favicon` | Entry list, detail — domain favicon with initial fallback |
| `TOTPDisplay` | Entry detail — live TOTP code with countdown ring |
| `PasswordGenerator` | Entry modal — random/diceware with sliders |
| `ToastContainer` | Global — stackable notifications (top-right) |
| `TagContextMenu` | Sidebar — right-click edit/delete on tags |

---

## State Management

### Global State (`AppStateContext`)

Single React Context providing all vault state and actions:

| State | Type | Description |
|-------|------|-------------|
| `entries` | `PasswordEntry[]` | All entry previews |
| `selectedEntry` | `PasswordEntry \| null` | Currently selected entry (decrypted) |
| `tags` | `Tag[]` | Tag definitions |
| `settings` | `AppSettings` | All app preferences |
| `searchTerm` | `string` | Current search query |
| `filterCategory` | `FilterCategory` | Active filter (all / favorites / tag) |

### Backend Abstraction

```
Component → useAppState()
              → backend.ts (interface)
                → tauri-backend.ts (Tauri invoke implementation)
                  → commands.rs (Rust)
```

Rule: **No Tauri API imports in visual components.** All backend calls go through `useBackend()` or `useAppState()`.

### Conversion Layer

Two mapping functions in `AppStateContext.tsx`:
- `entryPreviewToPasswordEntry()` — list-level mapping (no decryption)
- `decryptedEntryToPasswordEntry()` — detail-level mapping (full decrypt)

---

## Routing

| Route | Component | Purpose |
|-------|-----------|---------|
| `/` | `VaultSelect` | Choose or create a vault |
| `/login` | `LoginScreen` | Master password entry |
| `/app` | `AppLayout` | Main three-panel interface |

---

## Animation System

All animations use Framer Motion (`motion.div`, `AnimatePresence`).

| Animation | Trigger | Implementation |
|-----------|---------|----------------|
| Panel slide | Settings open/close | `translateX` with `AnimatePresence` |
| List stagger | Entries load | `variants` with `staggerChildren` |
| Content fade | Entry select | `opacity` + `y` transition |
| Copy flash | Button click | 800ms `setTimeout` icon toggle |
| Error shake | Wrong password | `x` keyframe array |
| Modal enter | Dialog open | `scale` + `opacity` |
| Skeleton pulse | Loading state | CSS `@keyframes` background oscillation |

---

## Theming

CSS custom properties on `:root` with `data-theme="dark|light"`:

```css
--bg-primary, --bg-elevated, --bg-hover
--text-primary, --text-secondary, --text-tertiary
--border, --border-subtle
--accent, --accent-hover
```

System preference detection via `matchMedia('prefers-color-scheme: dark')`. All component colors reference CSS variables — no hardcoded hex codes.

---

## Performance

- **Search**: debounced 150ms, trigram index in memory
- **Favicons**: lazy loaded with colored-initial fallback
- **Entry list**: lightweight `EntryPreview` (no decryption until selected)
- **Trash cleanup**: automatic 30-day expiry on vault save
- **Vault payload**: MessagePack (~20% larger than bincode, 5x smaller than JSON)

---

## Frontend Lifecycle Invariants

- **Rules of Hooks**: Zero early returns prior to hook declarations. All hooks execute unconditionally at top-level on every render.
- **Lexical Scoping**: Functions and callbacks are declared prior to reference in effects or other hooks to prevent Temporal Dead Zone (TDZ) runtime errors.
- **Conditional Mounting**: Components that require platform runtime availability (e.g. Tauri-only features like `SmartLoginButton`) are conditionally mounted from the parent rather than altering hook counts internally.

