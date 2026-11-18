# Vertex Browser: migration to Servo

## Goal

Replace the WebView2/Tauri runtime with a native Rust desktop application embedding Servo. The existing browser remains intact while the new runtime is developed on `servo-migration`.

## Current repository findings

The current application is not just a Tauri window. Its React interface depends on Tauri IPC, while external pages are rendered in child WebView2 instances managed by `src-tauri/src/webview_manager.rs`.

The existing Rust services are valuable and should be evaluated for reuse rather than rewritten blindly:

- `src-tauri/src/db.rs`: SQLite persistence.
- `src-tauri/src/adblock.rs`: filter-list parsing and rule storage; the WebView2 request interception integration must be replaced.
- `src-tauri/src/password_manager.rs`: encrypted vault logic; page-level credential detection/autofill needs a new integration.
- `src-tauri/src/downloads.rs`: download persistence and transfer logic; engine download events need an adapter.
- `src-tauri/src/commands.rs` and `lib.rs`: Tauri command/runtime glue, not portable as-is.
- `src-tauri/src/memory_saver.rs`: currently coupled to Tauri and WebView2 tab lifecycle; redesign around Servo WebViews and measured engine behaviour.

The current React frontend uses `@tauri-apps/api` and `vortexApi.ts`; it cannot simply be hosted in the native window without introducing another webview engine. The native browser chrome therefore needs a native UI (initial candidate: egui), or a separately designed UI process with a clearly defined IPC boundary.

## Native Servo branch status (2026-11-18)

The `native/` application is the active migration target on `servo-migration`. It uses a native `winit` window, Servo WebViews and an `egui` browser toolbar. Do not reintroduce Tauri or WebView2 into this runtime.

Implemented in the native branch:

- Native window, Servo rendering/event-loop integration, address bar and tab UI.
- Navigation, page title updates, zoom controls and keyboard shortcuts.
- SQLite-backed history, bookmarks, settings and a downloads-history table.
- Internal pages for history, bookmarks and download records.
- Basic request blocking through Servo's `WebViewDelegate::load_web_resource`.
- Global AdBlock toggle and per-host exceptions, persisted in SQLite.
- Native custom-rule editor with SQLite persistence. The supported EasyList subset includes domain rules (`||host^`), domain/path rules and exception rules (`@@||host^`); comments are ignored. Cosmetic filters, regular expressions, resource modifiers, scriptlets and full uBlock syntax are not supported.

Known limitations (not yet acceptance-tested):

- The native AdBlock module implements a deliberately limited EasyList network-rule subset plus a small built-in host list. It is not EasyList/uBlock-compatible: cosmetic filters, regular expressions, resource modifiers, scriptlets and filter-list updates are not implemented.
- Rules are parsed into an in-memory rule set when the browser starts and when custom rules are saved, rather than reparsed for every resource request. Path rules with the EasyList separator marker check the separator boundary, so `/ads^` does not accidentally match `/adserver`.
- The downloads page currently displays persisted records only. Actual network transfers, progress reporting, cancellation and opening files are not yet integrated.
- Servo's pinned `WebViewDelegate::load_web_resource` hook exposes the outgoing `WebResourceRequest` and lets the embedder replace a response; it does not expose the eventual server response headers to that callback. Therefore it cannot safely identify `Content-Disposition: attachment` downloads on its own. Do not intercept every URL or start a second unauthenticated HTTP request as a workaround. A proper download integration needs a verified response/download event or an engine-level API change.
- Password storage/autofill, tab session restore, robust tab lifecycle and Windows packaging remain unfinished.
- No successful native build or end-to-end browser test has been recorded yet. Do not treat source-level changes as proof of runtime correctness.

## Migration sequence

1. **Engine proof of concept (current):** start Servo in a native winit window, render a page, handle resize/redraw and event-loop wakeups. Keep this separate from the existing app.
2. **Input and navigation:** forward mouse, wheel, keyboard, focus and clipboard events; implement URL navigation and loading/title callbacks.
3. **Native browser chrome:** build address bar, tabs, toolbar, menus and dark theme with egui or another native toolkit. Do not embed React in WebView2.
4. **Browser services:** adapt SQLite, bookmarks, history, encrypted vault and downloads behind engine-independent Rust interfaces.
5. **Privacy and ad blocking:** integrate request interception using Servo's network/resource APIs, with per-site allow/deny controls. Verify API coverage before claiming EasyList parity.
6. **Tab lifecycle:** implement multiple Servo WebViews, session restore, suspend/discard policies and process/resource accounting.
7. **Compatibility and release:** test real sites, forms, media, WebGL, permissions, downloads, TLS, accessibility and Windows packaging.
8. **Benchmark:** compare against Chrome on the same machine and URLs at 1/5/10/20 tabs. Record RAM working set/private bytes, CPU time, startup and page-load timings. Do not claim a speed or memory improvement until measured.

## Acceptance criteria for the first prototype

- Runs as a native Windows process with no Tauri runtime and no WebView2 dependency.
- Opens a remote HTTPS page and presents Servo-rendered frames.
- Resizes correctly and exits cleanly.
- Does not modify the existing `src-tauri/` runtime or the `main` branch.

## Important constraints

Servo is an engine, not a drop-in Chromium replacement. Its embedding API and web-platform coverage are evolving. DRM, extensions, WebRTC, DevTools, media codecs and other compatibility features must be tested individually. A smaller process footprint or faster page load is a benchmark hypothesis, not a guaranteed result.
