# GLib 0.18.5 security backport

Source: the unmodified crates.io `glib` 0.18.5 distribution (gtk-rs, MIT; see LICENSE and COPYRIGHT). Registry archive SHA-256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`. This copy retains its published version to remain compatible with Tauri's GTK3 dependency graph.

The only Rust source change is the upstream fix for [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html): `VariantStrIter::impl_get` now uses a mutable output pointer and passes `&mut p` to `g_variant_get_child`. Exact upstream commit: [b5a4071e439bef2b5eea76c3aa25e5ae84839e34](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34), merged in [PR 1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343). No ABI, format or dependency changes were made.

Root Cargo.toml selects this copy with `[patch.crates-io]`. Version-only scanners may continue to report 0.18.5: the source patch, rather than a fictitious version bump or global advisory suppression, is the mitigation. Remove this patch when the Tauri Linux stack supports a fixed maintained GLib line (0.20 or later). Linux execution and the upstream GLib tests require GLib development libraries and remain separate from Windows verification.

The registry's `.cargo-ok` and `.cargo_vcs_info.json` cache bookkeeping is not distributed. All package source, tests, benches, build metadata and licensing are included.
