# Vortex Browser — native Servo migration

Vortex is being migrated to a **native Rust desktop browser** with Servo as its web engine. The migration is developed on the `servo-migration` branch; the legacy `main` branch is intentionally preserved until the native implementation is validated.

## Architecture target

- **Runtime:** native Rust process; no Tauri runtime, WebView2, Edge WebView, CEF, or other system webview.
- **Web engine:** Servo embedded in-process.
- **Window and input:** `winit`.
- **Browser chrome:** native Rust UI using egui/Glow on Servo's shared OpenGL context.
- **Persistence:** SQLite, reusing the existing schema and SQL logic after removing Tauri-specific coupling.
- **Ad blocking:** retain filter-list parsing and per-site rules; replace WebView2 request interception with a Servo-supported network integration.
- **Passwords:** retain the Argon2id/XChaCha20-Poly1305 vault core; rebuild page-aware save/autofill integration around supported Servo APIs.
- **Downloads:** adapt the existing transfer manager to native events and Servo download requests.

## Current status

The native prototype in `native/` creates a desktop window, embeds Servo, paints frames, handles resizing/DPI, and forwards mouse, wheel, keyboard, focus, and committed IME input. It has a first egui/Glow browser-chrome overlay with a visible tab strip, address/search field and navigation controls, plus multi-tab shortcuts, a SQLite profile, history/bookmarks pages and a Servo-rendered new-tab page.

This is an **early engine proof of concept, not yet a complete browser**. The chrome is currently an overlay and the page viewport has not yet been inset below it. Download integration, password autofill, ad blocking, settings, session restore, installer and production hardening remain to be implemented and tested. No performance improvement is claimed until measured against Chrome on the same machine.

See:
- [Servo migration plan](docs/SERVO_MIGRATION.md)
- [Migration audit and implementation order](docs/SERVO_MIGRATION_AUDIT.md)
- [Native runtime instructions](native/README.md)

## Build the native prototype on Windows 11 x64

Prerequisites:
- Windows 11 x64
- Visual Studio 2022 Build Tools with **Desktop development with C++**
- Git and the Rust toolchain specified in `native/rust-toolchain.toml`

From the repository root, in PowerShell:

```powershell
cd native
cargo check
cargo test
cargo run -- https://example.com
```

The first Servo build is large and can take a substantial amount of time. The Windows GitHub Actions workflow also runs `cargo check` and the navigation parser tests on pushes and pull requests to `servo-migration`.

## Existing browser services

The old implementation remains in `src-tauri/` and `src/` for reference during migration. Do not launch it as the target browser and do not add new WebView2 dependencies to the native runtime. Portable service logic should be extracted deliberately and covered by tests before the legacy runtime is retired.
