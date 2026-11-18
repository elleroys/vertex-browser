# Vortex Browser — native Servo runtime

Vortex is a Windows desktop browser built around **Servo embedded directly in a native Rust process**. The active implementation is in `native/` on the `servo-migration` branch. The target runtime does not use Tauri, WebView2, Edge WebView, CEF, or another system webview.

> **Status:** development/testing build. Core browser features are implemented in source, but the native Windows build and runtime still require successful CI and hands-on validation. Do not treat this branch as a production-ready browser yet.

## Current native features

- Servo rendering in a native `winit` window, with egui/Glow browser chrome.
- Tabs, address/search bar, navigation controls, keyboard shortcuts, new-tab page, session restore and recently closed tabs.
- SQLite profile, browsing history, bookmarks and configurable search/homepage.
- Download manager for direct HTTP(S) URLs, with progress, cancellation and safe resume when server validators permit it.
- Encrypted local password vault using Argon2id and XChaCha20-Poly1305, with explicit same-origin autofill.
- Native AdBlock subset: custom rules, per-site exceptions, local/remote lists and scheduled refresh. This is not full EasyList/uBlock compatibility.
- Site permission prompts and persistent per-origin decisions.
- Site-data management for cookies, localStorage and sessionStorage. IndexedDB, Cache Storage and service-worker cleanup are not yet covered.
- Navigation safety checks, IDN/Punycode and mixed-script warnings, plus optional local OpenPhish Community URL matching.
- Configurable privacy settings and persistent profile data.

See [native/README.md](native/README.md) for the detailed feature inventory and known limitations.

## Build on Windows 11 x64

### Requirements

- Windows 11 x64.
- Visual Studio 2022 Build Tools or Visual Studio with **Desktop development with C++** workload.
- Git.
- Rust toolchain from [native/rust-toolchain.toml](native/rust-toolchain.toml). Rustup will select it automatically when you enter `native/`.

### Build and run

Open PowerShell in the repository root:

```powershell
.\scripts\build-windows.ps1
```

The script builds the optimized native executable and creates `dist\Vortex-Windows-x64.zip`.

To build and immediately launch it:

```powershell
.\scripts\build-windows.ps1 -Launch
```

To launch a specific page:

```powershell
.\scripts\build-windows.ps1 -Launch -Url "https://example.com"
```

Manual development commands:

```powershell
cd native
cargo check
cargo test
cargo run -- https://example.com
```

The first Servo build downloads and compiles a large engine and may take a long time. Ensure that the Visual C++ build tools are installed and that the machine has sufficient free disk space.

## Downloading a CI build

After the Windows GitHub Actions workflow succeeds, download the `Vortex-Windows-x64` artifact from the workflow run:

[GitHub Actions — servo-migration](https://github.com/elleroys/vertex-browser/actions/workflows/servo-windows.yml?query=branch%3Aservo-migration)

The artifact is a portable ZIP containing `Vortex.exe` and a short run guide. It is a development build and may still have platform/runtime limitations.

## Security and privacy notes

- OpenPhish Community integration is disabled by default and is restricted by the provider to personal/academic/independent non-commercial use. Commercial product use requires a separately licensed source and written permission where applicable.
- OpenPhish checks are local: visited page URLs are not sent to the feed provider. Downloading the feed itself exposes the client's network connection to OpenPhish.
- IDN, mixed-script and HTTP warnings are risk signals, not proof that a site is malicious. Users can encounter false positives.
- The native AdBlock parser intentionally supports only a documented subset of filter-list syntax.

## Architecture

- Runtime and browser services: Rust.
- Web engine: pinned Servo revision.
- Window/input: winit.
- Browser chrome: egui/Glow.
- Profile persistence: SQLite.
- Security-sensitive secrets: Argon2id + XChaCha20-Poly1305.

The legacy Tauri/React implementation remains in the repository for reference only. New browser-runtime work belongs in `native/`.

## Validation policy

Source commits are not proof that the app compiles or works. The Windows CI workflow runs `cargo check`, `cargo test` and a release build. Real Windows startup, browsing, downloads, media, password autofill, site compatibility and performance still need hands-on testing.
