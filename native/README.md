# Vertex Native — Servo prototype

This directory is the isolated native-engine proof of concept. It does not replace or delete the existing Tauri application.

## Current scope

- Native desktop window created by `winit`.
- Servo is embedded in-process through its current `ServoBuilder` / `WebViewBuilder` API.
- Initial page can be supplied as the first command-line argument; defaults to `https://example.com`.
- Window resize, DPI scaling, redraw, Servo event-loop wakeups and frame presentation are wired.\n- Mouse movement, clicks, leaving the viewport and wheel scrolling are forwarded to Servo.

This is an engine integration spike, **not yet the full Vertex browser**. Navigation controls, keyboard/mouse forwarding, multiple tabs, downloads, permissions, profile storage, ad blocking and the browser chrome are not implemented here yet.

## Toolchain and Windows

Servo's current main branch requires a recent Rust toolchain (the upstream toolchain file currently pins Rust 1.99.1; use that version for this pinned Servo revision). Use the upstream-pinned toolchain for reproducible builds.

Servo's Windows build has native dependencies and platform-specific rendering configuration. In particular, the Windows dependency enables Servo's `no-wgl` feature. Media support requires the matching GStreamer development/runtime libraries; this initial prototype intentionally does not enable the media feature.

From the repository root:

```powershell
cd native
cargo run -- https://example.com
```

The first build is expected to be large and may take a long time because Servo compiles a substantial engine and JavaScript runtime.

## Upstream API references

- Servo crate API: https://doc.servo.org/servo/
- Servo embedding overview: https://book.servo.org/embedding/overview.html
- Upstream minimal winit embedder: https://github.com/servo/servo/blob/main/components/servo/examples/winit_minimal.rs
- Servo source revision pinned by this prototype: `32fc710d5b0eb4bd27d4f3b5317a0f5f97b3bf17`

The embedding API is still evolving. Keep the Servo revision pinned and update it deliberately after checking upstream API changes.
