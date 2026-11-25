# Architecture

## Module graph

```
            ┌────────────────────────────┐
            │ ui (GPUI chrome)           │  feature ui-shell
            │  tab strip · toolbar ·     │
            │  omnibox · menu · settings │
            └──────────┬─────────────────┘
                       │ commands            ▲ effects
                       ▼                     │
            ┌──────────────────────────────────────────┐
            │ browser.rs (session controller)           │
            │  validate → mutate → persist → effects    │
            └───────┬──────────────────────┬────────────┘
                    │                      │
      ┌─────────────▼─────────┐   ┌────────▼──────────────────────┐
      │ domain (pure model)   │   │ infra                          │
      │  tab · space · layout │   │  db (SQLite, migrations)       │
      │  settings · omnibox   │   │  ipc (strict validation)       │
      │  command + allowlist  │   │  paths (profile, spaces root)  │
      └───────────────────────┘   └────────▲───────────────────────┘
                                           │ validated events
            ┌──────────────────────────────┐
            │ webview (wry host)           │  feature webview-host
            │  per-space WebContext        │
            │  internal pages · bridge js  │
            └──────────────────────────────┘
```

Nothing above `infra`/`domain` depends on a toolkit. The controller
(`browser.rs`) never calls GPUI or wry; adapters consume its `Effect`
values. That is what keeps 84 tests green on a bare toolchain.

## Session flow

1. The chrome dispatches a `BrowserCommand` (typed enum).
2. The controller validates it: known ids, scheme allowlist, feature
   flags (`require` fails closed).
3. State mutates; every committed change is flushed to SQLite (WAL, one
   small transaction per command).
4. The controller returns `Effect`s; the window executes the ones it owns
   (`AttachWebview`, `LoadUrl`, `DetachWebview`, devtools, window open).

Webview reports take the mirrored path: the wry IPC handler decodes with
`infra::ipc::decode_webview_message` before anything reaches the
controller (`report_navigation`, `report_open_request`).

## Surfaces are per tab

One native webview per tab, keyed by `TabId`. Panes are viewports: the
reconciliation pass positions the focused pane's tab over the content
area and hides the rest. Tab history therefore survives pane swaps,
split closes, and tear-off drags — a dragged tab is detached and
re-created in the new window because native surfaces cannot reparent.

## Isolation model

- One `wry::WebContext` per space, rooted at `<profile>/spaces/<space>`.
- `SpaceId` parsing rejects anything outside `[a-z0-9-]{1,32}`, so the
  id cannot traverse paths.
- Webviews of different spaces never share storage origins, even for the
  same site.
- Top-level `data:` navigations are refused at the wry navigation handler
  as defense in depth behind the domain allowlist.

## IPC threat model

Everything a page sends is hostile input:

| control | mechanism |
| --- | --- |
| size | 64 KiB cap before parsing |
| shape | tagged enums, `deny_unknown_fields` |
| urls | scheme allowlist (`http`, `https`, `file`, `phantom`) |
| text | length caps, control-character rejection |
| feedback | static error strings, no payload echoes |
| window.open / target=_blank | bridge converts to validated `open_in_new_tab` |

## State schema (v1)

```
meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)          -- settings blob
windows(id, x, y, width, height, maximized, active_tab_id, layout_json)
tabs(id, window_id → windows ON DELETE CASCADE, space_id, title, url, position)
```

`layout_json` is the serialized split tree; its pane→tab references are
validated against the tabs table on load, and inconsistent windows are
skipped at restore instead of blocking launch.

## Known phase-2 seams

- Keyboard shortcuts fire when chrome has focus; keys over the page go to
  the page (WebView2 owns them) — an accelerator bridge is phase 2.
- Omnibox typing works per keystroke; IME composition and clipboard paste
  in the omnibox are phase 2 (domain buffer already supports arbitrary
  inserts).
- gpui's published API moves quickly; `ui/` isolates every assumption in
  small render helpers so API drift is a local fix.
