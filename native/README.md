# Vertex Native — Servo browser runtime

This directory contains the native browser runtime prototype. The rendering engine is Servo embedded directly in the Rust process. The native path must not use Tauri, WebView2, or another system webview.

## Implemented in this prototype

- Native desktop window created with `winit`, with an egui/Glow browser-chrome overlay sharing Servo's OpenGL context.
- Servo embedded in-process through `ServoBuilder` and `WebViewBuilder`.
- Initial navigation from the first command-line argument; without an argument, the browser opens its Servo-rendered new-tab page.
- New-tab page with a Google search form, up to eight bookmark shortcuts and eight recently visited pages.
- Input parsing for HTTP(S) URLs, dotted hostnames, and search queries.
- Window resize, DPI scaling, redraw, Servo event-loop wakeups and frame presentation.
- Mouse movement, clicks, leaving the viewport and wheel scrolling forwarded to Servo.
- Keyboard down/up events, logical keys, common physical key codes, and modifier state forwarded to Servo.
- Window focus is synchronized with the active Servo WebView.
- IME committed text is forwarded to Servo.
- Native shortcuts: Ctrl+L enters an editable address/search field (type, Ctrl+A to clear/replace, Enter to navigate, Escape to cancel), Ctrl+R / F5 reload, Alt+Left back, Alt+Right forward.
- Toolbar actions include bookmark toggle, history and bookmarks pages in addition to back/forward/reload/home and address navigation.
- Multiple Servo WebViews in one native window, with active-tab focus and visibility management.
- Tab lifecycle shortcuts: Ctrl+T opens the new-tab page, Ctrl+W closes the active tab (the final tab is kept open), Ctrl+Tab switches forward and Ctrl+Shift+Tab switches backward; Ctrl+1 through Ctrl+8 activate the corresponding tab and Ctrl+9 activates the last tab.
- Active page title is reflected in the native window title.
- A per-user SQLite profile is created under `%APPDATA%\\VertexBrowser\\profile.sqlite3` on Windows (or a home-directory fallback elsewhere).
- Browsing history is recorded on navigation and updated when the page title arrives; internal `about:` pages are excluded.
- Ctrl+H opens a dark, Servo-rendered browsing history page (up to 500 recent URLs).
- Ctrl+Shift+B opens a dark, Servo-rendered bookmarks page; links navigate directly in the active WebView.
- Ctrl+D toggles a bookmark for the current HTTP(S) page. Bookmark/history/settings storage is implemented as an engine-independent native service, with APIs for recent history, listing bookmarks, deleting one history entry and clearing history.

## Still missing

This is still an early engine shell, not yet a complete consumer browser. The current chrome is a first integrated egui/Glow overlay: it shares Servo's OpenGL context and paints after the page. The page viewport has not yet been inset below the toolbar, so the page may render underneath the chrome area. Downloads, password autofill, ad-block network interception, profile settings, session restore, installer and production hardening remain missing. Do not claim those features are implemented.

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


The toolbar overlay uses an opaque dark backing over its top 82 logical pixels to prevent page content from bleeding through. Servo's WebView still occupies the full rendering context; true viewport insetting remains future rendering work.
