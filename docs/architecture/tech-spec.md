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
- **Network and response limits**: Core network policy gates HTTP requests and cancels them when networking is disabled. Vault bodies and WebDAV discovery responses have bounded sizes; redirects remain disabled.
- **Memory Protection**: Decrypted payload buffers zeroed using `Zeroize`. Remote credentials held strictly in volatile in-memory registry (`sessionSecrets.ts`), wiped upon vault lock.

### Peer-to-Peer (P2P) Direct Sync & Zero-Knowledge Device Pairing

#### 1. Background Wi-Fi Sync (Port 5322)
- **Opt-in LAN synchronization**: Paired devices exchange encrypted vault contents when network access and sync are enabled. Disabling networking cancels in-flight operations.
- **Protocol v3 authentication (`YNSYN003`)**: A shared-vault HMAC proves membership, while a separately generated installation Ed25519 key proves the enrolled device identity. Both sides authenticate a length-prefixed, role-labelled transcript containing both UUIDs, salt commitments, fresh challenges, and ephemeral P-256 public keys. The listener checks the client first; the client verifies the listener before uploading entries.
- **Fresh transfer encryption**: Ephemeral P-256 ECDH plus HKDF-SHA256 derives each session key. XChaCha20-Poly1305 AAD binds the handshake transcript and direction. A shared vault key alone cannot decrypt a captured v3 transfer.
- **Fail-closed enrollment**: Missing, duplicate, legacy, or revoked device keys require explicit re-pairing. An empty trusted list authorizes no peers. Ordinary sync never imports the remote device trust policy; there is no legacy shared-key-only fallback.
- **Revocation boundary**: Revoking a peer blocks future authenticated sync on that installation. It cannot erase copies or keys the peer already acquired. Shared inner vault keys are not rotated by revocation, so a former peer may still decrypt a separately obtained compatible snapshot.

#### 2. PIN Device Pairing (Port 5324)
- **Pairing keys**: BLAKE3 domain separation, Argon2id, and HKDF-SHA512 derive pairing keys from the master password and six-digit PIN. Reusing those inputs reuses the derived keys; fresh challenges and authenticated transfer AAD distinguish sessions.
- **Explicit enrollment**: Client-first mutual HMAC authentication precedes role-labelled device metadata protected by a MAC over both challenges. Metadata includes each UUID and Ed25519 public key; both records enter payload AAD. Successful pairing explicitly registers those keys for later v3 sync.
- **Cancellation**: Pairing uses cancellable accept/I/O loops and absolute deadlines; native callbacks recheck cancellation before installing active or pending vault state. Pairing hosts refresh disk revision before merging completed data into the open manager.
- **Adopt mode**: An unauthenticated client reads no existing local vault or entries and sends zero entries. Its installation identity still participates in authentication. Imported vaults use collision-free paths, including Windows reserved-name sanitization. Pairing an existing vault is restricted to the currently open vault.

#### 3. QR Device Pairing (Transport `YQR3`)
- **QR secret**: A 256-bit random secret, UUID session, advertised listener address, and visual SAS are encoded in the unchanged `yntrapair://v2` URI format with a 90-second lifetime. No password or database is in the QR. Possession of the QR secret grants pairing authority during that lifetime.
- **Transport**: `YQR3` and `PAIR3_OK` reject legacy metadata exchange. Mutual HMAC authentication, fresh challenges, and authenticated device metadata bind the UUIDs and signing public keys. XChaCha20-Poly1305 payload AAD binds the QR session and full metadata transcript.
- **Native secret handling**: A received password is removed before the result crosses IPC. Passwordless adoption retains native vault data and sync keys until a local wrapping password is selected. Lock and cancellation clear pending state. The local wrapping password need not equal the host's master password for a protected replica.
- **Migration**: Both peers must update and re-pair to enroll signing keys. Loss or replacement of an installation signing key also requires re-pairing. This optical bootstrap still transfers data over the network and is not an air-gapped protocol.
#### 4. Active UDP Query-Response Discovery (Port 5323)
- **Active Query-Response Protocol**: Clients emit periodic `YQRY` query pulses (`[YQRY (4B) | pairing_beacon_id (32B)]`). Hosts respond with immediate direct unicast `YPAR` packets (`[YPAR (4B) | pairing_beacon_id (32B) | local_port (2B)]`), allowing direct replies when the network permits them; discovery cannot bypass access-point isolation.
- **In-Loop Self-Echo Isolation**: The receive loop in `listen_discovery_beacon` evaluates received packet source IP addresses against known local network adapters and loopback *inside* the receive loop. Self-echo packets are dropped immediately, allowing the listener to continue until remote peers respond or the timeout expires.
- **Multi-Interface Local Network Enumeration & IPv4 Prioritization**: `get_local_lan_ips` probes all active network adapters (Ethernet, Wi-Fi, mobile hotspots, virtual adapters), excludes un-routable link-local IPv6 addresses (`fe80::/10`) and multicast, and sorts IPv4 addresses first to prefer common LAN endpoints; reachability still depends on routing and firewall policy.
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

### Favicon Resolution & Caching Engine (`Favicon.tsx` / `crates/core/src/services/favicon.rs`)
- **Privacy Gate**: The user setting controls external favicon resolution; the native host starts network access denied and applies settings before enabling optional requests. The current UI default enables website icons, so requested domains are disclosed to the configured providers when enabled.
- **Session-Only Cache**: The frontend and native service retain at most 250/256 validated data URIs in memory. Legacy localStorage entries and native disk caches are removed and never loaded or written; lock and setting changes invalidate in-flight work and purge the memory cache.
- **Image MIME & Signature Validation**: Responses capped at 512 KB and validated against image MIME types and binary magic headers (PNG, ICO, SVG, WebP, GIF, JPEG), rejecting HTML/JSON error or fallback pages.
- **SSRF & Private Network Shield**: Strictly drops resolution attempts for loopback, private LAN (`10/8`, `172.16/12`, `192.168/16`), cloud metadata (`169.254.169.254`), and internal/anonymity TLDs (`.local`, `.lan`, `.internal`, `.home`, `.corp`, `.onion`, `.i2p`).
- **Concurrency Throttling**: Frontend queue bounds active IPC fetches to `MAX_CONCURRENT_FETCHES = 4`; Rust uses `tokio::sync::Semaphore::new(6)` to eliminate TCP socket starvation on large entry counts.
- **Provider Fallback Chain**: (1) DuckDuckGo ICO CDN, (2) Google s2 API, then a validated parent/base domain fallback for subdomains with no root icon. Direct-host requests are not made.
- **Transient Failure Cooldowns & Auto-Recovery**: Failed resolutions trigger a 30-second cooldown (`failedCooldowns`) rather than permanent `null` caching. Cooldowns are reset automatically upon browser `online` events or when toggling the setting.
- **Title Fallback**: Automatically extracts domains from entry titles when the URL field is blank.

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

