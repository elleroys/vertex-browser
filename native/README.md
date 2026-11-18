# Vertex Native — Servo browser runtime

This directory contains the native browser runtime prototype. The rendering engine is Servo embedded directly in the Rust process. The native path must not use Tauri, WebView2, or another system webview.

## Implemented in this prototype

- Native desktop window created with `winit`, with an egui/Glow browser-chrome overlay sharing Servo's OpenGL context.
- Servo embedded in-process through `ServoBuilder` and `WebViewBuilder`.
- Initial navigation from the first command-line argument; without an argument, the browser opens its Servo-rendered new-tab page.
- New-tab page with a Google search form, up to eight bookmark shortcuts and eight recently visited pages.
- Input parsing for HTTP(S) URLs, dotted hostnames, and search queries.
- Window resize, DPI scaling, redraw, Servo event-loop wakeups and frame presentation.
- A Servo `OffscreenRenderingContext` renders the page into a framebuffer sized to the content area below the 82-logical-pixel browser chrome; the frame is blitted into the native window before egui paints the toolbar. Mouse coordinates are translated into page-viewport coordinates and toolbar-area pointer events are not sent to the page.
- Mouse movement, clicks, leaving the viewport and wheel scrolling forwarded to Servo.
- Keyboard down/up events, logical keys, common physical key codes, and modifier state forwarded to Servo.
- Window focus is synchronized with the active Servo WebView.
- IME committed text is forwarded to Servo.
- Native shortcuts: Ctrl+L enters an editable address/search field (type, Ctrl+A to clear/replace, Enter to navigate, Escape to cancel), Ctrl+R / F5 reload, Alt+Left back, Alt+Right forward.
- Toolbar actions include bookmark toggle, history and bookmarks pages in addition to back/forward/reload/home and address navigation.
- Multiple Servo WebViews in one native window, with active-tab focus and visibility management. Pages opening auxiliary windows via `window.open` create native tabs; page-requested `window.close` closes its tab when another tab remains.
- Tab lifecycle shortcuts: Ctrl+T opens the new-tab page, Ctrl+W closes the active tab (the final tab is kept open), Ctrl+Tab switches forward and Ctrl+Shift+Tab switches backward; Ctrl+1 through Ctrl+8 activate the corresponding tab and Ctrl+9 activates the last tab.
- Recently closed HTTP(S) tabs can be reopened with Ctrl+Shift+T or the ↶ toolbar button; up to ten URLs are kept in memory for the current run. Each tab also has a ⋯ context menu for closing it or reopening the most recently closed tab.
- Active page title is reflected in the native window title.
- A per-user SQLite profile is created under `%APPDATA%\\VertexBrowser\\profile.sqlite3` on Windows (or a home-directory fallback elsewhere).
- Browsing history is recorded on navigation and updated when the page title arrives; internal `about:` pages are excluded.
- Ctrl+H opens a dark, Servo-rendered browsing history page (up to 500 recent URLs); the native history manager supports opening entries, deleting individual entries and confirmed full-history clearing.
- Ctrl+Shift+B opens a dark, Servo-rendered bookmarks page; the native bookmarks manager supports opening and deleting saved bookmarks.
- Ctrl+D toggles a bookmark for the current HTTP(S) page. Bookmark/history/settings storage is implemented as an engine-independent native service, with APIs for recent history, listing bookmarks, deleting one history entry and clearing history.
- A persistent browser session snapshot stores up to 50 HTTP(S) tab URLs and the active-tab index in SQLite. On startup without an explicit URL argument, the browser restores that session; malformed, obsolete and non-HTTP(S) entries are ignored.
- Up to ten recently closed HTTP(S) tabs are persisted across restarts and can be reopened with Ctrl+Shift+T or the toolbar recovery control.
- A basic settings window lets the user select Google, DuckDuckGo or Bing as the address-bar/new-tab search provider, configure an HTTP(S) homepage, choose a persistent download directory through the native folder picker, enable/disable startup session restore and optionally clear browsing history on exit; preferences are stored in the local profile.
- A local encrypted password vault using Argon2id and XChaCha20-Poly1305, with create/unlock/lock, search, save/delete, password generation and clipboard copy.
- Servo-backed login-form detection and explicit user-triggered same-origin autofill. Autofill checks the current origin again immediately before filling, serializes values safely and does not submit the form.
- Password vault CSV import/export with a preview/validation step before import confirmation, row/size limits and temporary-buffer zeroization. The supported CSV schema is Vertex's own `origin,title,username,password,notes` format; arbitrary Chrome/vendor exports are not guaranteed to import unchanged.
- A limited native AdBlock request interceptor with global enable/disable, per-site exceptions and custom domain/path/exception rules persisted in SQLite, including `*` wildcards inside path patterns. The rule editor can import local EasyList-style text files up to 5 MiB, de-duplicates lines and reports how many imported rules this matcher supports versus ignores. It can also fetch a user-supplied remote list in a background thread: HTTPS only (including redirects), a 25-second request timeout, a 5 MiB response limit, UTF-8 validation and a requirement for at least one supported rule. Successfully fetched rules replace the previous remote list (without deleting the user's separate custom rules), are saved to the profile and applied after the active page reloads. A configured list is refreshed automatically at browser startup; the same action can be triggered manually from the rule editor. It is not a full EasyList parser: cosmetic rules, regexes, resource modifiers and scriptlets are not applied.
- Right-clicking an HTTP(S) link opens a native link-actions menu with **Download link…** (native Save As dialog) and **Open in new tab**. This uses the pinned Servo context-menu element metadata; it does not pretend to detect attachment responses or automatically download on a normal left click.
- A native download manager for direct HTTP(S) URLs: native save-as dialog, streamed writes to a stable partial file, progress and resume validators persisted in SQLite, cancellation, retry with HTTP Range/If-Range using a strong ETag or Last-Modified when the server supports validators (otherwise it restarts from byte zero), open completed files and reveal their folder. If a server ignores Range or the representation is stale (HTTP 200/416), the transfer safely restarts from byte zero rather than appending incompatible data. Active transfers left by a previous process are marked failed on startup, while partial files are retained for manual resume. Redirects are limited to HTTP(S), capped at ten hops, and HTTPS-to-HTTP downgrade is blocked. The manager does not yet automatically capture attachment links clicked inside web pages because the pinned Servo WebView delegate exposes request metadata but not completed response headers.

## Still missing

This remains a development browser, not a release-ready consumer browser. The following work is still required:
- A complete settings/profile interface (privacy controls and cookie/site-data management). Search provider, HTTP(S) homepage, persistent download-directory selection and startup session-restore preference are implemented, but the broader settings surface is not.
- Richer session metadata (scroll position and per-tab navigation history). Current session restore stores only HTTP(S) URLs and the active tab index; recently closed URLs are separately persisted.
- Broader AdBlock compatibility: scheduled periodic filter-list updates, cosmetic filters, resource modifiers, scriptlets, and full EasyList/uBlock syntax. The current matcher supports domain rules, path prefixes, `*` wildcards in paths and exception rules, but not the full filter-list grammar.
- More complete browser security/privacy controls, permission prompts, cookie/site-data management and clipboard auto-clear policy.
- Windows installer/update packaging, icons/signing, crash handling, accessibility and production hardening.
- Site compatibility and performance measurements on real Windows hardware. Servo feature coverage (media, WebRTC, DRM, extensions and DevTools) must be evaluated separately.

Do not treat source-level implementation or GitHub commits as proof that the application compiles or works at runtime. Build and end-to-end testing are intentionally deferred until the feature set is implemented.

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


The page surface is an offscreen framebuffer attached to the window's Servo rendering context. Its height excludes the 82-logical-pixel toolbar area, and a framebuffer blit places it below the toolbar before the native chrome is drawn.
