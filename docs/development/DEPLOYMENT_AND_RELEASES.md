# Yntra Vault Deployment & Release Guide

This document details the dual-repository architecture, git push procedures, and release automation pipelines for Yntra Vault.

---

## 1. Dual-Repository Architecture

Yntra Vault is maintained using a dual-repository model to separate proprietary development artifacts, internal specifications, and AI assistant guidelines from clean, public open-source releases:

1. **Private Development Repository (`yntra-vault-private`)**:
   - Location: `c:\Users\cdfvgbhnjnmkl\yntra-vault-private-main`
   - Remote: `https://github.com/YntraAB/yntra-vault-private.git`
   - Contains: Complete commit history, full test harness, `.agents/` instructions, internal drafts, and developer specifications.
2. **Public Release Repository (`yntra-vault`)**:
   - Location: `c:\Users\cdfvgbhnjnmkl\yntra-vault`
   - Remote: `https://github.com/YntraAB/yntra-vault.git`
   - Contains: Clean, verified production source code, public documentation (`README.md`), reproducible build manifests, and GitHub Actions CI/CD workflows. All `.agents/` and development-only files are strictly excluded.

---

## 2. Working in the Private Repository

When adding features, fixing security issues, or writing tests, work in the private repository as normal:

```powershell
# In yntra-vault-private-main:
git status
git add .
git commit -m "feat(security): harden p2p discovery and transit AEAD encryption"
git push origin main
```

---

## 3. Publishing a Release to the Public Repository

Releases to the public repository are automated via the root PowerShell script `publish-public.ps1`.

### Command:
```powershell
# Run from the private repository root:
.\publish-public.ps1 -DestDir "..\yntra-vault" -Push
```

### What `publish-public.ps1` Does:
1. **Sanitizes Destination**: Cleans the destination directory (preserving `.git`).
2. **Permitted Directory Copy**: Copies `src`, `crates`, `src-tauri`, `fuzz`, `public`, `scripts`, `.cargo`, `.github`, and `docs`.
3. **Excludes Private Artifacts**: Excludes `.agents`, `target`, `node_modules`, `dist`, `.tmp`, and debug logs.
4. **Renames Documentation**: Copies `docs/README-public.md` to `README.md` in the public repository root.
5. **Creates Release Commit**: Stages all files, commits with message `"Release Yntra Vault v<version>"`.
6. **Pushes & Tags**: Pushes `main` to GitHub and creates/pushes an annotated tag `v<version>`.

---

## 4. Multi-Platform Release Automation (GitHub Actions)

When a version tag (`v*`) is pushed to the public repository, the workflow in `.github/workflows/release.yml` triggers automatically:

1. **Desktop Bundles (`build-desktop`)**:
   - **Windows**: Produces Windows installers (`.msi`, `.exe`).
   - **Linux**: Produces Linux `.AppImage` (standalone portable executable) and `.deb`.
   - **macOS**: Produces macOS `.dmg`.
2. **Android APK (`build-android`)**:
   - Runs on an Ubuntu runner with Java 17, Android SDK, and NDK.
   - Compiles Rust targets (`aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, `i686-linux-android`).
   - Generates universal standalone Android APK (`.apk`).
   - Attaches the APK directly to the GitHub Release assets.

---

## 5. Verification Checklist Before Publishing

Before executing `publish-public.ps1 -Push`:
- [ ] Ensure `bun run build` succeeds with zero TypeScript errors.
- [ ] Ensure `cargo test --manifest-path crates/core/Cargo.toml` passes (all 129 tests).
- [ ] Ensure version in `package.json`, `Cargo.toml`, and `CHANGELOG.md` are aligned.
