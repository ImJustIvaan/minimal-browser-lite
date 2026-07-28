# Minimal Browser Lite (Tauri edition)

The super-lightweight sibling of the Electron "Minimal Browser". Uses the
OS-native webview instead of bundling Chromium:

| OS | Engine |
|---|---|
| Windows | WebView2 (Chromium-based, MS-maintained, ships with Win10/11) |
| macOS | WKWebView (WebKit/Safari engine) |
| Linux | WebKitGTK |

Expect a **few-MB installer** and **~20-40MB idle RAM**, vs. Electron's
100MB+ baseline — because it reuses the engine already on the OS instead of
shipping its own copy.

## The trade-off: no Chrome extensions

WKWebView and WebKitGTK have no Chromium extension API at all. WebView2 on
Windows technically has an experimental extension-loading API, but wiring
that up would make extensions Windows-only — inconsistent with a browser
meant to behave the same on all three OSes. So this edition ships **no
extension support**, full stop. If you need real cross-platform Chrome
extensions, use the Electron version in `../Minimal-Browser` instead.

## Status: unverified scaffold

No Rust toolchain was available in the environment that generated this, so
`src-tauri/src/main.rs` has **not been compiled**. It's written against the
Tauri 2.x multiwebview APIs (`Window::add_child`, `WebviewBuilder`,
`on_navigation`) from documentation, but exact method names/signatures can
drift between Tauri point releases. Budget time to fix small API mismatches
on first `cargo build`.

## What's implemented

- Tab strip + toolbar UI, visually matching the Electron version (Chrome-style
  rounded tabs, pill address bar).
- Tabs backed by real child webviews (`add_child`), not iframes — each tab is
  an isolated native webview.
- Back/forward/reload via injected `history.back()/forward()` and
  `location.reload()` JS (Tauri's webview doesn't expose native back/forward
  handles the way Electron's BrowserView does).
- History and bookmarks, persisted as JSON in the app's data dir.
- Tab titles kept in sync via a `MutationObserver` injected into each tab
  that reports `document.title` back to Rust.

## What's intentionally left out vs. the Electron version

- No extensions (see above).
- No find-in-page (Tauri's webview doesn't expose Chromium's `findInPage`;
  would need a custom in-page JS search injected per tab).
- No background-tab suspension — all tabs' webviews stay alive (parked
  off-screen at zero size when inactive). Native webviews are already much
  lighter per-instance than Electron's per-tab renderer processes, so this
  matters less here.

## Setup

```
# Install Rust: https://rustup.rs
# Install Tauri CLI + platform prerequisites: https://v2.tauri.app/start/prerequisites/
npm install
npm run dev      # cargo tauri dev — opens a live dev window
npm run build     # cargo tauri build — produces installers under src-tauri/target/release/bundle
```

Platform prerequisites (installed once per machine, same idea as before):

- **Windows**: WebView2 runtime (preinstalled on Win11, auto-installed on
  Win10 by the Tauri installer), MSVC Build Tools.
- **macOS**: Xcode command line tools.
- **Linux**: `webkit2gtk`, `libayatana-appindicator3`, etc. — see the Tauri
  prerequisites link above for your distro's package names.

Build per OS on that OS — Tauri doesn't cross-compile the webview bindings.
