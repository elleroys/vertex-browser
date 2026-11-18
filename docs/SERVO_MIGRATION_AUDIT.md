# Vortex native Servo implementation audit

Audit branch: `servo-migration`  
Runtime source: `native/`  
Audit date: 2026-11-18

## Executive summary

The branch contains a substantial native browser implementation, not merely an engine proof of concept. The application embeds Servo in a native winit process and uses egui/Glow for browser chrome. Core services for tabs, persistence, downloads, passwords, ad blocking, permissions and navigation safety are present in source.

However, source presence is not proof of a working executable. The pinned Servo build is large, and the Windows CI workflow is the authoritative compile/test/release gate. A manual Windows 11 startup and feature smoke test is still required before calling this usable.

## Feature inventory

| Subsystem | Current implementation | Remaining validation or work |
|---|---|---|
| Engine/window | Servo embedded in winit; egui/Glow toolbar; offscreen page rendering | Windows startup, resize, DPI, shutdown and GPU compatibility |
| Navigation/input | HTTP(S) navigation, search parsing, keyboard/mouse/wheel/focus and committed IME | Broader key mapping, IME composition and real-site checks |
| Tabs | Multiple WebViews, tab switching/closing, shortcuts, auxiliary windows, recently closed tabs | Full per-tab history and scroll/session metadata |
| Profile | SQLite settings, bookmarks, history, sessions and downloads | Migration/backup/restore UX and profile management |
| Downloads | Native HTTP(S) streaming, progress, cancellation, retries, partial-file persistence and conditional resume | Automatic page attachment integration and runtime testing |
| Password vault | Argon2id + XChaCha20-Poly1305, lock/unlock, save/search/delete, generator, CSV import/export | Real-site form compatibility, clipboard policy and recovery UX |
| AdBlock | Rule subset, custom rules, per-site exceptions, local import, HTTPS remote lists and scheduled refresh | Full EasyList/uBlock syntax, cosmetic filters, scriptlets and broader modifiers |
| Permissions | Prompt, allow once, persistent allow/block and revoke manager | Test every supported Servo permission type |
| Site data | Cookies, localStorage and sessionStorage list/clear | IndexedDB, Cache Storage and service-worker cleanup |
| Navigation security | Scheme/credential/downgrade hard blocks; IDN, mixed-script, invisible-control, IP, deep-subdomain and HTTP warnings | False-positive tuning and more threat intelligence |
| Threat feed | Optional OpenPhish Community exact-URL matching, strict parsing, local cache, 12-hour refresh | Commercially licensed source for commercial distribution; provider feed has limited coverage |
| Packaging | PowerShell build/package script and CI portable ZIP workflow | CI success, Windows smoke test, installer, signing and updater |

## Security boundaries and caveats

- OpenPhish Community is opt-in and disabled by default. Its provider terms restrict it to personal/academic/independent non-commercial use. Do not use it in commercial distribution without appropriate written permission or a separately licensed feed.
- Visited URLs are matched locally and are not sent to OpenPhish. Fetching the feed itself makes a request to the provider.
- Threat-feed matches are exact canonical URL matches, not domain-wide blocks.
- IDN/mixed-script/HTTP signals are warnings and may have false positives. HTTPS alone does not establish that a site is trustworthy.
- The AdBlock parser implements a subset, not full filter-list compatibility.
- Unsupported external protocols are blocked rather than launched by the OS.

## Build and release gate

The workflow `.github/workflows/servo-windows.yml` targets Windows x64 and is configured to run:

1. `cargo check`
2. `cargo test`
3. `cargo build --release`
4. Package `Vortex.exe` into `Vortex-Windows-x64.zip`
5. Upload the portable ZIP as a GitHub Actions artifact.

The artifact is available only when the workflow reaches a successful completion. A pending, cancelled or failed workflow is not a verified build.

## Manual smoke test after downloading

1. Extract the ZIP and launch `Vortex.exe`.
2. Open HTTPS sites and search terms; verify address bar and page title.
3. Create, switch, close and restore tabs.
4. Add/remove bookmarks and inspect browsing history.
5. Change a setting and restart to verify profile persistence.
6. Test download progress, cancellation and resume with a server that supports range requests.
7. Create/unlock the password vault and test autofill only on a test account and same-origin form.
8. Verify permission prompts and site-data clearing.
9. Test AdBlock rules and navigation warning cases.
10. Inspect logs/crashes and confirm no profile or download data is lost.

Performance claims and production readiness require separate repeatable tests.
