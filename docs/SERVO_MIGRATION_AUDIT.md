# Servo migration audit

Audit target: `servo-migration` at the time of this commit.
The existing `main` branch and its Tauri/WebView2 implementation are intentionally left unchanged.

## Executive summary

The repository already contains a real Servo embedding spike, not yet a usable browser.
The native executable creates a `winit` window, builds Servo and one WebView, loads an initial URL, paints frames, handles DPI/resize, and forwards mouse movement, clicks and wheel events.

The current native prototype does **not** yet provide keyboard input, an address bar, browser navigation controls, tabs, browser chrome, persistence, downloads, password autofill or ad blocking. Do not describe it as a completed browser or claim performance gains.

## Existing implementation inventory

| Area | Existing implementation | Migration decision |
|---|---|---|
| Browser UI | React/TypeScript; calls Tauri IPC through `src/bridge/vortexApi.ts` | Rebuild as native UI. Do not host this UI in WebView2 or add a second browser engine. |
| Web rendering | `src-tauri/src/webview_manager.rs`; Tauri child WebViews backed by WebView2 | Replace completely with Servo WebViews. |
| SQLite | `src-tauri/src/db.rs`; schema for bookmarks, history, sessions, downloads, settings, vault and adblock | Reuse the schema and SQL operations after removing Tauri-specific types. Put behind an engine-independent service. |
| Ad blocking | `src-tauri/src/adblock.rs`; adblock-rust rules plus WebView2 request interception | Reuse filter parsing, list storage and whitelist logic. Replace the request interception layer with a Servo network/resource integration. Verify the actual Servo API before promising EasyList parity. |
| Password vault | `src-tauri/src/password_manager.rs`; Argon2id and XChaCha20-Poly1305 | Reuse cryptographic storage core. Rebuild page detection, save prompts and autofill around Servo-supported embedder/content APIs. |
| Downloads | `src-tauri/src/downloads.rs`; reqwest streaming, pause/resume and Tauri events | Reuse transfer logic after replacing `AppHandle`/events with a native event channel and wiring Servo download requests. |
| Memory saver | `src-tauri/src/memory_saver.rs`; WebView2/Tauri lifecycle | Redesign around Servo WebView lifecycle and measured memory behaviour; do not port the current suspend calls. |
| Runtime glue | `src-tauri/src/commands.rs`, `lib.rs`, `tauri.conf.json` | Retire only after native equivalents are implemented. Keep the files on `main` during migration. |
| Local server | `server.ts` | Reassess after native UI/service boundaries are defined; it is not the browser engine. |

## Native prototype findings

- `native/Cargo.toml` pins a specific Servo revision, which is appropriate while the embedder API is changing.
- `native/src/main.rs` creates one native window and one Servo WebView.
- The current event handler forwards mouse movement, mouse buttons and wheel input. Keyboard and IME input are absent.
- There is no browser UI toolkit in the native crate yet.
- The pinned Servo build is expected to be large. Windows native dependencies and the Rust toolchain must be validated on a Windows machine; a GitHub source audit cannot establish that it compiles.

## Recommended implementation order

1. **Finish the engine shell:** keyboard/IME/focus input, back/forward/reload/stop, URL entry, title/loading state, safe URL-vs-search parsing, and clean shutdown.
2. **Add native browser chrome:** choose egui for the first integrated prototype; implement dark theme, address bar, toolbar, tab strip and basic menus in the same native process.
3. **Introduce service boundaries:** extract database, bookmarks, history and settings from Tauri runtime types into plain Rust services. Keep storage format compatible.
4. **Integrate downloads:** define an engine-independent download event/request interface, then connect Servo's download callbacks to the existing transfer manager.
5. **Integrate ad blocking:** first prove interception coverage with a small test fixture, then add filter-list refresh, per-site allow/deny and counters.
6. **Integrate password manager:** retain the encrypted vault; implement origin-aware save/fill only through supported Servo APIs and explicit user consent.
7. **Tabs and sessions:** create/destroy/switch Servo WebViews, persist session state and measure resource use before adding discard policies.
8. **Validation and packaging:** Windows x64 build, real-site compatibility, forms, downloads, TLS, accessibility, media support and installer.
9. **Benchmark:** compare against Chrome on the same machine and repeatable URLs at 1/5/10/20/30 tabs. Record RAM working set/private bytes, CPU time, startup and page-load timings. Publish raw results; do not promise a percentage improvement in advance.

## Definition of done for the first integrated native milestone

- Native Windows executable with no Tauri runtime and no WebView2 dependency.
- Address bar can navigate to HTTPS URLs and search terms.
- Keyboard, mouse, wheel, focus and basic IME work.
- Back, forward, reload and stop work.
- Page title and loading state are reflected in native chrome.
- Resizing and clean shutdown work.
- Existing `main` branch remains intact until the native path is reviewed and tested.
