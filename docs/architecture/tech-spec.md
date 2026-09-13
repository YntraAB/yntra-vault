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

### Backend (Rust)

| Crate | Purpose |
|-------|---------|
| `argon2` | KDF — Argon2id (memory-hard, GPU-resistant) |
| `chacha20poly1305` | Vault-level AEAD (XChaCha20-Poly1305) |
| `aes-gcm` | Entry-level AEAD (AES-256-GCM) |
| `hkdf` + `sha2` | Subkey derivation (HKDF-SHA512) |
| `hmac` | P2P handshake authentication & legacy v1/v2 file verification |
| `p256` | Passkey generation (ECDSA P-256 / ES256) |
| `zeroize` | Memory safety — automatic sensitive data clearing |
| `rmp-serde` | Vault payload serialization (MessagePack, v2/v3 format) |
| `clap` + `clap_complete` | Command line argument parsing & shell completion generator |
| `ratatui` + `crossterm` | Interactive Terminal UI (TUI) rendering & terminal event loop |
| `rpassword` | Terminal interactive password prompt |
| `comfy-table` | Terminal table output formatting |
| `colored` | Terminal ANSI color highlighting |

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
| `SettingsPanel` | Slide-in overlay: General, Appearance, Security, WebDAV Cloud Sync, Backup tabs |
| `SecurityDashboard` | Audit results: weak/reused/old/breached password breakdown |

---

## Synchronization Protocol (WebDAV & P2P)

### WebDAV Cloud Sync
- **Optimistic Concurrency**: Uses HTTP `If-Match` headers with normalized ETags (RFC 7232).
- **Conflict Resolution**: On HTTP 412 (Precondition Failed), executes up to 3 optimistic retry attempts: fetches remote ETag, downloads remote payload, performs item-level 3-way merge with tombstone preservation, saves locally, and retries conditional PUT.
- **Transport Security**: Enforces HTTPS scheme for non-localhost endpoints (`http://` allowed only for `localhost`/`127.0.0.1`).
- **Memory Protection**: Zeroes intermediate decrypted remote vault buffers using `Zeroize` after 3-way merge deserialization.

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
