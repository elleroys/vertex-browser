# Vortex Browser: native Servo migration

## Target architecture

Vortex is being migrated to a native Rust desktop browser. The active implementation is `native/` on `servo-migration`.

- Runtime: native Rust process.
- Web engine: Servo embedded in-process at the pinned revision in `native/Cargo.toml`.
- Window/input: winit.
- Browser chrome: egui/Glow.
- Profile: SQLite under the per-user application data directory.
- Constraint: do not reintroduce Tauri, WebView2, Edge WebView, CEF or a second system webview into the native runtime.

The old Tauri/React code remains for reference. It is not the executable being developed on this branch.

## Implemented native functionality

The detailed source-of-truth inventory is [native/README.md](../native/README.md). Current implementation includes:

- Servo window, page rendering, event-loop integration, resize/DPI and mouse/keyboard/focus/committed IME input.
- Native toolbar, address/search field, tab strip, multiple WebViews, tab shortcuts, tab recovery, title/loading indicators and internal new-tab/history/bookmark pages.
- SQLite profile, settings, bookmarks, history, session restore and recently closed tabs.
- Download manager for direct HTTP(S) URLs with streamed writes, progress, cancellation, retry and conditional range resume where supported by server validators.
- Encrypted local password vault (Argon2id + XChaCha20-Poly1305), CSV import/export, login form detection and explicit same-origin autofill.
- Request blocking with a documented subset of EasyList-style rules, custom rules, per-site exceptions, local import and HTTPS remote list refresh.
- Permission prompts with per-origin persistent allow/deny decisions and a permission manager.
- Site data manager for cookies, localStorage and sessionStorage.
- Navigation hardening for unsupported schemes, embedded credentials and HTTPS-to-HTTP downgrade.
- Heuristic warnings for IDN/Punycode, mixed-script homographs, invisible/bidi controls, literal IP hosts, deep subdomains and non-local HTTP.
- Optional OpenPhish Community feed with strict parsing, local SQLite cache and exact URL matching.

## Known limitations

This is a development browser, not a production-ready release.

- No successful end-to-end acceptance test on a user's Windows machine has yet been recorded. Use the latest Windows CI result as the source of truth for compile/test status.
- The current AdBlock matcher is not a full EasyList/uBlock parser. Cosmetic filters, scriptlets, regexes and several modifiers are not supported.
- Download manager supports direct HTTP(S) URLs and right-click link actions; it does not yet automatically capture every attachment response initiated by a page.
- Site-data clearing currently covers cookies and Web Storage only, not IndexedDB, Cache Storage or service workers.
- Session persistence stores tab URLs and active index, not full per-tab back/forward stacks or scroll positions.
- External/custom protocols are blocked; safe OS handoff is not implemented.
- Media, DRM, WebRTC, extensions, DevTools, accessibility and broad site compatibility need separate validation against the pinned Servo build.
- Windows packaging is a portable development ZIP, not a signed installer or auto-updater.
- OpenPhish Community is restricted by the provider to personal/academic/independent non-commercial use. It is disabled by default; commercial product use requires an appropriately licensed source and any required written permission. Do not enable it for commercial distribution without resolving the license.

## Windows build and validation

Requirements:
- Windows 11 x64.
- Visual Studio 2022 / Build Tools with Desktop development with C++.
- Rustup and the exact toolchain declared in `native/rust-toolchain.toml`.

From the repository root:

```powershell
.\scripts\build-windows.ps1
```

This builds the optimized native executable and packages `dist\Vortex-Windows-x64.zip`. Add `-Launch` to run it after building, or `-Launch -Url "https://example.com"` to open a page.

Manual checks:

```powershell
cd native
cargo check
cargo test
cargo build --release
cargo run -- https://example.com
```

The first Servo build is large and can take a long time. The workflow at [GitHub Actions](https://github.com/elleroys/vertex-browser/actions/workflows/servo-windows.yml?query=branch%3Aservo-migration) runs check, unit tests, release build and uploads a portable Windows x64 artifact when all steps succeed.

## Acceptance checklist before hands-on testing

- [ ] Windows CI completes `cargo check`.
- [ ] Windows CI completes `cargo test`.
- [ ] Windows CI completes `cargo build --release` and uploads `Vortex-Windows-x64`.
- [ ] Downloaded executable starts on Windows 11 x64.
- [ ] HTTPS navigation, search, tab creation/switch/close and session restore work.
- [ ] History, bookmarks, settings and profile persistence work.
- [ ] AdBlock, permissions and navigation warnings behave as documented.
- [ ] Download cancellation/retry and password vault workflows work.
- [ ] No critical crashes or data-loss issues during a basic browsing session.

Do not report performance improvements until they are measured against Chrome on the same machine and repeatable pages/workloads.
