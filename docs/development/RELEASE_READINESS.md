# Yntra Vault Release Readiness & Hardening Specification

This document defines the architectural guidelines, failure modes, and verification procedures to ensure that Yntra Vault builds run reliably, without panics, crashes, or feature regressions in `--release` production environments.

---

## 1. Overview & Problem Statement

In desktop application development (particularly with Rust and Tauri/React), features may appear to function correctly in development (`debug`) mode but fail or crash in production (`release`) mode. 

These discrepancies arise from fundamental runtime and compiler differences:
- **Panic Behavior**: Debug mode unwinds the stack and outputs backtraces; release mode aborts (`panic = "abort"`).
- **Subsystem Environment**: Debug mode attaches a console; release mode detaches `stdout`/`stderr` on Windows (`windows_subsystem = "windows"`).
- **Content Security Policy (CSP)**: Development mode uses a local Vite server; release mode packages assets into custom webview schemes (`tauri://` or `http://ipc.localhost`).
- **React Minification**: In development, React logs console warnings for hook irregularities; in production, React throws fatal `Minified React error #300` / `#310` if hook call counts differ between renders.
- **Optimization & LTO**: Release compilation uses aggressive Link-Time Optimization (`lto = true`, `opt-level = 3`), which can alter memory layouts, inline aggressively, and eliminate dead stores.

This specification details each failure vector and outlines the mandatory invariants to prevent them.

---

## 2. Root Causes & Prevention Invariants

### 2.1 Panic Semantics: `panic = "abort"` vs Unwinding

In `Cargo.toml`, the release profile specifies:
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

#### The Failure Mode
In debug mode, Rust defaults to stack unwinding (`panic = "unwind"`). If a non-critical component encounters an `.unwrap()` error (e.g. system tray icon loading or clipboard initialization), the panic might be caught or surface as an error log. In release mode, **any unhandled panic immediately aborts the process** with exit code `0xC0000005` or similar, offering zero UI feedback to the user.

#### Prevention Invariant
1. **Zero-Unwrap Policy on System Resources**: Never call `.unwrap()` or `.expect()` on window icons, system trays, clipboard handlers, environment variables, or filesystem paths.
2. **Safe Fallbacks**:
   ```rust
   // INCORRECT (crashes if icon is missing or resolution fails):
   let tray = tray_builder.icon(app.default_window_icon().unwrap().clone()).build(app)?;

   // CORRECT:
   if let Some(icon) = app.default_window_icon() {
       tray_builder = tray_builder.icon(icon.clone());
   }
   let tray = tray_builder.build(app)?;
   ```
3. **Error Propagation**: Return `Result<T, VaultError>` or `Result<T, String>` across all Tauri command boundaries using the `?` operator.

---

### 2.2 Windows Subsystem Disconnection

In `src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

#### The Failure Mode
When compiled for release on Windows, the application runs under the GUI subsystem without an attached console window. Standard input/output handles (`std::io::stdin`, `stdout`, `stderr`) are null or invalid (`INVALID_HANDLE_VALUE`). 

If third-party libraries or internal routines write to unbuffered standard streams without checking validity, calls may fail or block.

#### Prevention Invariant
1. Do not use raw `println!` or `eprintln!` for operational status.
2. Use structured logging (`tracing` / `log`) or internal error channels that redirect to memory buffers or Tauri frontend event emitters.
3. For CLI binaries (`yntra-cli`), do not apply `windows_subsystem = "windows"`; CLI tools must retain console attachments.

---

### 2.3 Tauri v2 CSP & Custom Protocol IPC

In `src-tauri/tauri.conf.json`, Tauri enforces a Content Security Policy (CSP) on webview traffic:
```json
{
  "app": {
    "security": {
      "csp": "default-src 'self'; img-src 'self' asset: https://asset.localhost blob: data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost https://ipc.localhost https://api.pwnedpasswords.com;"
    }
  }
}
```

#### The Failure Mode
During development, the frontend loads from `http://localhost:1420`, and IPC messages traverse Vite web sockets and HTTP bridges. In production packages, Tauri serves frontend assets through internal custom protocols:
- `http://ipc.localhost` (Windows / Linux WebKit)
- `https://ipc.localhost` (Windows WebView2 secure scheme)
- `tauri://localhost` (macOS WebKit)

If `connect-src` specifies `http://ipc.localhost` but omits `https://ipc.localhost`, the webview silently blocks IPC invoke requests (`invoke(...)`), causing buttons, forms, and vault unlocks to hang indefinitely without console errors.

#### Prevention Invariant
The CSP `connect-src` directive MUST always include both `http://ipc.localhost` and `https://ipc.localhost` alongside `ipc:` and `'self'`.

---

### 2.4 React Hook Ordering & Minified Errors (#300 / #310)

Production frontend bundles built by Vite are minified with Terser/esbuild, stripping verbose React development warnings.

#### The Failure Mode
React enforces strict hook execution order. If a component executes hooks conditionally (e.g. after an early `if (!entry) return null;` check or inside a conditional branch):
- Development mode logs a warning in the console: *"React has detected a change in the order of Hooks called by..."*.
- Release mode throws **`Minified React error #300`** or **`#310`**, instantly unmounting the entire component tree and leaving a blank white screen.

#### Prevention Invariant
1. **Unconditional Top-Level Hooks**: Every React hook (`useState`, `useEffect`, `useCallback`, `useMemo`, `useRef`, custom hooks) MUST be placed at the absolute top of the component body.
2. **No Early Returns Before Hooks**: No `return` statement may precede any hook declaration.
3. **Lexical Ordering**: Functions and state referenced inside effects or callbacks must be declared in topological order to prevent temporal dead zone (TDZ) reference errors.

```tsx
// INCORRECT (Hook called after early return):
export function EntryView({ entry }: { entry: PasswordEntry | null }) {
    if (!entry) return <EmptyState />;
    const [isEditing, setIsEditing] = useState(false); // CRASH in release!
    return <div>...</div>;
}

// CORRECT:
export function EntryView({ entry }: { entry: PasswordEntry | null }) {
    const [isEditing, setIsEditing] = useState(false); // Unconditional
    if (!entry) return <EmptyState />;
    return <div>...</div>;
}
```

---

### 2.5 SPA Routing Strategy: HashRouter Invariant

In `src/main.tsx`, the application router uses:
```tsx
import { HashRouter } from 'react-router-dom';
```

#### The Failure Mode
`BrowserRouter` relies on HTML5 `pushState` and web server URL rewriting (where any route like `/settings` falls back to `index.html`). In desktop release packages, files are served from static virtual file roots. Requesting `/settings` directly will cause the webview to request `tauri://localhost/settings` or `http://ipc.localhost/settings` as a file, resulting in an HTTP 404 / file-not-found error.

#### Prevention Invariant
The desktop application MUST always use `HashRouter`. URL state is preserved in the URL hash fragment (`/#/settings`), which never issues an external HTTP request to the webview's asset resolver.

---

### 2.6 Cryptographic Dead Store Elimination Surviving LTO

When Link-Time Optimization (`lto = true`) is enabled with `opt-level = 3`, LLVM performs cross-crate dead store elimination.

#### The Failure Mode
If sensitive keys or plaintext passwords in memory are zeroed using standard compiler-optimizable operations (e.g. `for b in slice { *b = 0; }` or naive `memset`), the optimizer observes that the memory buffer is never read again before being freed. It subsequently removes the zeroing loop as a dead store, leaving master passwords intact in process RAM.

#### Prevention Invariant
1. All sensitive buffers in `yntra-crypto` use `LockedBuffer`.
2. Memory zeroing MUST utilize `core::ptr::write_volatile` followed by `core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst)`.
3. Intermediate decrypted subkeys and transit credentials must be wrapped in `zeroize::Zeroizing<T>` or `ProtectedSecret`.

---

### 2.7 Test Mock & Harness Gating: The `cfg!(debug_assertions)` Trap

When writing mock hooks for hardware security modules (e.g. YubiKeys, TPMs, or Windows Hello biometrics), code must support automated CI runs and cross-crate integration tests.

#### The Failure Mode
If a test mock or harness hook checks `cfg!(debug_assertions)`:
```rust
// BUG: When integration tests run under `cargo test --release`, debug_assertions is FALSE!
fn is_hardware2fa_mock_enabled() -> bool {
    cfg!(test)
        || (cfg!(debug_assertions) && MOCK_HARDWARE_2FA.load(Ordering::Relaxed))
        || (cfg!(debug_assertions) && std::env::var("YNTRA_TEST_MODE").is_ok())
}
```
1. `cfg!(test)` is only defined within the crate currently undergoing unit tests. For library dependencies being compiled for an external integration test (e.g. `crates/core/tests/vault_lifecycle.rs`), `cfg!(test)` evaluates to `false`.
2. Under `cargo test --release`, `cfg!(debug_assertions)` evaluates to `false`.
3. Consequently, the mock flag is completely ignored in release mode even though the test explicitly called `set_hardware2fa_mock(true)`. The code falls through to communicate with real hardware, triggering `Hardware2FaNotAvailable` errors and panicking in release mode while succeeding in debug mode!

#### Prevention Invariant
1. Mock hooks must check their atomic control flags (`MOCK_HARDWARE_2FA.load(...)`) and test environment variables (`YNTRA_TEST_MODE`) directly, without wrapping them in `cfg!(debug_assertions)`.
2. Hardware mock APIs must only be exposed internally and never surfaced through public Tauri IPC commands or CLI arguments.

---

## 3. Pre-Release Verification Protocol

Before releasing or tagging any version of Yntra Vault, the following sequential verification protocol must be executed and achieve 100% pass status:

### Step 1: Frontend Type-Check & Production Build
```bash
bun run build
```
*Validates*:
- TypeScript compilation across all `.ts` and `.tsx` files (`tsc -b`).
- Production bundling and minification with Vite (`vite build`).
- Zero syntax, typing, or missing asset errors.

### Step 2: Frontend Unit Test Suite
```bash
bun test
```
*Validates*:
- All unit tests for cryptography helpers, recovery code sanitization, domain matching, and state stores pass.

### Step 3: Rust Workspace Release Check
```bash
cargo check --release --workspace
```
*Validates*:
- All 4 crates (`yntra-crypto`, `yntra-vault-core`, `yntra-cli`, `yntra-vault-app`) compile cleanly under `--release` profile flags without warnings treated as errors.

### Step 4: Rust Workspace Release Test Suite
```bash
cargo test --release --workspace
```
*Validates*:
- All cryptographic tests (Argon2id, XChaCha20-Poly1305, HKDF, Shamir secret sharing, TPM/DPAPI wrapping).
- Vault save/load/rekey pipelines with full compiler optimizations (`opt-level = 3`, `lto = true`).
- IPC daemon and constant-time authentication tests.

### Step 5: Full Package Build
```bash
bun run tauri build
```
*Validates*:
- Native bundle generation (MSI, NSIS on Windows; DMG on macOS; AppImage/Deb on Linux).
- Asset embedding and icon validation.

---

## 4. Release Readiness Checklist for AI Agents & Developers

When submitting changes, ensure every item on this checklist is satisfied:

- [ ] **Zero Unhandled Unwraps**: No `.unwrap()` or `.expect()` calls on system resources (icons, trays, clipboard, file handles).
- [ ] **CSP Completeness**: `tauri.conf.json` contains `http://ipc.localhost` and `https://ipc.localhost` in `connect-src`.
- [ ] **React Hook Discipline**: All hooks in `src/` are placed at the top level of components before any conditional branches or returns.
- [ ] **Router Integrity**: `HashRouter` is preserved in `src/main.tsx`.
- [ ] **Memory Scrubbing**: Sensitive buffers use volatile zeroing and `LockedBuffer` to prevent LTO dead store removal.
- [ ] **Subsystem Isolation**: GUI binaries enforce GUI subsystem; CLI binaries retain console standard streams.
- [ ] **Clean Compilation**: Both `bun run build` and `cargo test --release --workspace` pass with zero failures.
