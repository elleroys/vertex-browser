# Vortex engine boundary

This branch introduces a dedicated Cargo crate for the browser engine.

## Ownership

- `crates/vortex-engine` is the only crate that declares the upstream Servo Git dependency, revision, and engine feature set.
- The browser shell depends on `vortex-engine` through a local path dependency.
- The shell accesses the transitional Servo re-export through `vortex_engine::servo`.
- Keep Servo's pinned revision and feature selection in the engine crate. Do not add Servo as a direct dependency of the shell.

## Build behavior

Cargo builds the engine crate and its Servo dependency as a dependency of the shell. Once those artifacts are built, changes limited to shell source files should not invalidate/recompile Servo. Changes to the engine crate, Servo revision, enabled features, Rust toolchain, target, or build profile can still trigger engine recompilation.

The engine is not literally compiled once forever: it is compiled once per relevant target/profile/toolchain cache and rebuilt when its inputs change. Do not run `cargo clean` during normal shell development.

## Current migration stage

This is a first isolation boundary, not yet a stable binary ABI. The current shell still uses Servo types through the re-export. Follow-up work should progressively move lifecycle, webview creation, event forwarding, and rendering behind Vortex-owned engine interfaces.

## Build commands (from `native/`)

- Fast dependency/type check: `cargo check`
- Debug build: `cargo build`
- Full engine rebuild only when engine inputs change: normal Cargo fingerprinting decides what must rebuild; do not manually delete `target/`.

## Windows note

The current Servo feature set still includes `media-gstreamer`. The existing Windows build failure for `glib-sys` / missing `pkg-config` is not fixed by crate isolation. It is tracked as a separate platform dependency issue; do not silently remove media support as a workaround.
