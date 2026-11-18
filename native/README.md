# Vertex Native — Servo browser runtime

This directory contains the native browser runtime prototype. The rendering engine is Servo embedded directly in the Rust process. The native path must not use Tauri, WebView2, or another system webview.

## Implemented in this prototype

- Native desktop window created with `winit`.
- Servo embedded in-process through `ServoBuilder` and `WebViewBuilder`.
- Initial navigation from the first command-line argument.
- Input parsing for HTTP(S) URLs, dotted hostnames, and search queries.
- Window resize, DPI scaling, redraw, Servo event-loop wakeups and frame presentation.
- Mouse movement, clicks, leaving the viewport and wheel scrolling forwarded to Servo.
- Keyboard down/up events, logical keys, common physical key codes, and modifier state forwarded to Servo.
- Window focus is synchronized with the active Servo WebView.
- IME committed text is forwarded to Servo.
- Native shortcuts: Ctrl+L enters an editable address/search field (type, Ctrl+A to select all, Enter to navigate, Escape to cancel), Ctrl+R / F5 reload, Alt+Left back, Alt+Right forward.
- Multiple Servo WebViews in one native window, with active-tab focus management.
- Tab lifecycle shortcuts: Ctrl+T opens a blank tab, Ctrl+W closes the active tab (the final tab is kept open), Ctrl+Tab switches forward and Ctrl+Shift+Tab switches backward.
- Active page title is reflected in the native window title.
- A per-user SQLite profile is created under `%APPDATA%\\VertexBrowser\\profile.sqlite3` on Windows (or a home-directory fallback elsewhere).
- Browsing history is recorded on navigation and updated when the page title arrives; internal `about:` pages are excluded.
- Ctrl+D toggles a bookmark for the current HTTP(S) page. Bookmark/history/settings storage is implemented as an engine-independent native service, with APIs for recent history, listing bookmarks, deleting one history entry and clearing history.

## Still missing

This is an engine shell, not yet a complete consumer browser. Ctrl+L currently provides keyboard-driven address entry through the window title; a proper visible native address bar and toolbar are still missing. It does not yet have a visible tab strip, visible bookmarks/history UI, download integration, password autofill, ad-block network interception, profile settings, installer or production hardening. Do not claim those features are implemented.

Physical-key mapping currently covers letters, digits, navigation keys, modifiers and F1–F12; uncommon keys still fall back to `Unidentified`. IME committed text is supported, while composition/preedit events remain follow-up work.

## Toolchain and Windows

Servo's pinned source revision requires a recent Rust toolchain. The exact toolchain is declared in the workspace/native configuration. Servo's Windows build has native dependencies and platform-specific rendering configuration. Media support is not enabled in this initial prototype.

From the repository root:

```powershell
cd native
cargo run -- https://example.com
```

The first build is expected to be large and may take a long time because Servo compiles a substantial engine and JavaScript runtime. A successful GitHub source update does not prove that the project compiles: validate with `cargo check` and `cargo run` on Windows x64.

## Architecture rule

All browser UI and browser services must be native Rust components around Servo. Do not reintroduce Tauri, WebView2, Edge WebView, CEF, or a second embedded browser engine as a shortcut. Reuse existing database and cryptographic logic only after removing Tauri-specific runtime dependencies.

## Upstream references

- Servo crate API: https://doc.servo.org/servo/
- Servo embedding overview: https://book.servo.org/embedding/overview.html
- Upstream minimal winit embedder: https://github.com/servo/servo/blob/main/components/servo/examples/winit_minimal.rs
- Pinned Servo source revision: `32fc710d5b0eb4bd27d4f3b5317a0f5f97b3bf17`
