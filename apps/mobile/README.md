# Phantom (mobile)

Chrome-like mobile browser shell built with Flutter. Lives under
`apps/mobile/` in the Phantom monorepo. Built and tested locally with the
Flutter SDK pinned in `.metadata` (3.24.5 stable), no CI.

## Architecture

Layered, dependencies point inward:

- `lib/domain/` — pure Dart models and contracts: spaces, tab stacks,
  preferences, feature flags, search engines, omnibox resolution, ad-block
  rules, form snapshots, repository interfaces.
- `lib/infra/` — Hive-backed persistence. Every Space gets its own data
  directory (`<root>/spaces/<space-id>/`); boxes are closed and re-bound on
  every Space switch, so one Space can never read another Space's data.
- `lib/webview/` — WebView host contract plus the JavaScript bridge
  builders (developer-tools console/network feed, safe form capture and
  restore) and the Flutter binding.
- `lib/ui/` — Chrome-like shell: top omnibox, back/forward/reload, tab
  switcher with drag reorder, three-dot menu (Developer tools entry),
  developer-tools overlay, settings page, flag-gated split view.

## Security rules

- No keys or secrets anywhere in this tree.
- Per-Space data directories enforced by the persistence layer; storage is
  never shared across Spaces.
- Zero telemetry: the shell makes no background requests. Network activity
  happens only when the user navigates.
- Form drafts exclude password, hidden and file inputs on both the injected
  script side and the persistence side.

## Feature flags

`enable_split_screen`, `enable_ad_blocker`, `enable_auto_update` — all
default off, toggled on the settings page. The split view menu entry and
the update check are gated by their flags.

## State restoration

The tab stack (including per-tab history) is persisted through Hive after
every mutation and restored on launch. Form drafts are captured through an
injected script before leaving a page and re-applied when the same page
finishes loading again.

## Development

```bash
flutter pub get
flutter analyze
flutter test
```
