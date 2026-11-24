# Phantom — Desktop

Windows-first desktop shell of the Phantom browser: a GPUI chrome (tab
strip, omnibox toolbar, 3-dot menu, settings window) over wry webviews,
with per-space data isolation and SQLite session restoration.

## Layout

```
apps/desktop/src/
  domain/    pure model: tabs, spaces, commands, settings, split layout, omnibox
  infra/     SQLite store + migrations, profile paths, strict IPC contract
  webview/   wry host (per-space data dirs), internal pages, state bridge
  browser.rs session controller: commands in, effects out, persistence in between
  ui/        GPUI chrome: tab strip, toolbar, menu, settings window, bootstrap
  headless.rs no-toolkit smoke path (`phantom --headless`)
  main.rs    entrypoint (--profile-dir, --headless, --version)
```

Dependencies flow one way: `ui` and `webview` depend on `browser` and
`domain`/`infra`; the model never depends on a toolkit. `ui-shell` and
`webview-host` are opt-in features, so everything else builds and tests on
a bare toolchain:

```
cargo test --no-default-features     # 84 tests: model, ipc, session, pages
cargo check                          # full shell (needs the feature deps below)
```

## Platform notes

- **Windows is the phase-1 target.** wry rides WebView2 there; the final
  binary cross-builds with `mingw-w64` (see `docs/build-windows.md`).
- **Linux:** wry's Linux backend needs webkit2gtk and a GTK main loop,
  which the GPUI shell does not run. Building the full shell on Linux also
  needs `libwebkit2gtk-4.1-dev libgtk-3-dev` for the webview crate to
  compile. Model and persistence tests run everywhere without them.

## Spaces (webview isolation)

Every space owns a `WebContext` rooted at `<profile>/spaces/<space>`.
Cookies, site storage and caches live in that directory only — two spaces
visiting the same site are distinct storage origins, and no webview reads
another space's data. Space ids are strict (`[a-z0-9-]{1,32}`), so the id
can never traverse out of the spaces root.

## IPC contract

The only door from web content to the core is `infra::ipc`. Webviews post
small JSON messages (`navigation_state_changed`, `open_in_new_tab`); the
validator enforces a 64 KiB cap before parsing, tagged enums with
`deny_unknown_fields`, an https/http/file/phantom scheme allowlist, length
caps on url/title fields, and control-character rejection. Errors are
static strings — payload fragments never flow back to the page.

## Feature flags

| flag | effect | default |
| --- | --- | --- |
| `enable_tiling` | horizontal/vertical pane splits in the window | off |
| `enable_drag_tabs` | drag a tab out of the strip to open a new OS window | off |
| `enable_auto_updater` | background update checks (not implemented yet) | off |

Gating is enforced in the session (`FeatureFlags::require`), not just in
the UI, so a flag left off fails closed.

## State restoration

`phantom.db` (SQLite, WAL) stores windows (geometry, maximized, active
tab, split layout as JSON) and tabs (space, title, url, strip order) in a
versioned schema. Restore skips a window whose snapshot fails
consistency validation and still launches; ids continue after the restored
maximum so nothing is ever reused.
