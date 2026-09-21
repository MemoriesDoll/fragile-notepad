# Iced upstream integration

Reviewed 2026-09-21 against upstream master
`7abcb02ca81a0d33bfbaf9d37e5858a5d440cd8f` (320 commits after the previous pin).

The application pins `ddd7c42a9ba625b219e5e8062ff9be83eea467c5` (2026-06-25),
advancing 21 upstream commits from `38d19f8f5a2be93fd1fd74a2e81df680ce3c1cc2`.
This is a compatibility update, not a migration to current master. The next major
changes replace layout constraints, renderer scaling, and the highlighter API.
Current master also conflicts with our renderer handoff and text rendering patches.
Those API migrations need their own review, especially for progressive parsing.

The selected base includes these applicable upstream fixes:

- `e22dcf7f4`: use the GPU adapter's texture resolution limits on native platforms.
- `d7f5627a7`: skip undersized images without abandoning the rest of the image layer.
- `5c21f73a9`: correct crossed color channels in `Color::invert`.
- `e46f9c9e6`: expose GPU power preference. The application's handoff and its probe
  explicitly retain the previous HighPerformance preference.

The vendor patch also backports recent fixes without the intervening API migration:

- `b54f2c599` / `8caf9e44f`: do not retain an expired redraw deadline when merging
  event-loop control flow; this prevents the stale-deadline CPU spin.
- `8e05eade2` / `40339edb7`: recover Lost, Outdated, and Other surface errors at most
  once per second instead of repeatedly failing and redrawing every window.
  Keep strict handoff failure/rollback handling outside this normal recovery path.
  Reset the recovery timestamp when replacing a window's rendering state.

- `ca79fdb70`: preserve remaining input events when an overlay disappears during
  an event batch, and clear its cached layout. Prevents menu dismissal from
  swallowing subsequent releases or keystrokes. The adapted upstream regression
  test passes with this fix and fails against the original runtime.

- `7c6ce8789`: preserve the stronger mouse interaction across nested overlays.
  An inactive child no longer masks the parent menu cursor. A nested-overlay
  regression test fails before the fix and passes after it.

- `3c81aac2e`: retain scrollbar interaction status in widget-tree state when a
  dropdown rebuilds its scrollable. The regression covers hover styling after
  rebuilding and redraw on pointer exit; it fails before the fix and passes after.

- `d8dabb4ab`: suppress the content cursor while a scrollbar is grabbed, even
  when the pointer moves off the scrollbar. A regression test covers both axes
  and restores content interaction after release; it fails before the fix and
  passes after it.

`BASE_REVISION` and `fragile-notepad-iced.patch` are the reproducible source of the
vendor checkout. Run `scripts/setup-vendor.ps1 apply` (Windows) or
`scripts/setup-vendor.sh apply` (Linux) to reconstruct it in a fresh checkout.

## Backport selection

The follow-up review selected the four input/overlay fixes above because the
affected runtime paths are used by the application's menus, dropdowns, tabs,
and scrollable panels. They apply without the newer widget or renderer APIs.

Other reviewed changes are not required on this base:

- The newer text-input submit/paste fixes address the replacement input
  implementation; this base already checks focus and dispatches paste handlers.
- The negative image-coordinate regression was introduced by the newer pixel
  snapping changes; this base already keeps signed image bounds.
- Smooth scrolling and built-in editor changes do not apply to the custom
  document editor. Adopting those features would be a separate change.
- Resize notification on application scale changes is not needed by the app's
  current zoom, which changes editor font size instead of Iced application scale.
- New layout/overlay invalidation fixes depend on APIs absent from this base.

## Base update validation

- Windows: `cargo test --locked --all-targets` — 686 passed.
- Debian WSL 2: the same command — 695 passed.
- Vendor libraries: `cargo test --locked -p iced_winit -p iced_graphics -p iced_tiny_skia --lib` — 44 passed.
- Optimized application and backend-switch probe builds passed.
- Nine traced backend-switch cases passed: normal, multiple windows, resizing,
  closing during preparation/commit, and injected prepare/warm/commit/first-present failures.
- Linux graphical system-appearance and live-change checks passed under WSLg/X11,
  using an isolated settings portal; explicit overrides and no-portal fallback passed.
- Rapid scrolling of titanic.html completed with a successful software-to-GPU handoff.
- Applying the exported patch to a fresh checkout of the pin reproduces all 30
  patched files in the tested vendor tree.

The GPU probes test handoff success and rollback. Normal driver surface-error
recovery is an upstream backport; a real driver surface failure was not induced.

## Follow-up input/overlay backport validation

- Windows: `cargo test --locked --all-targets` — 686 passed.
- Debian WSL 2: the same command — 695 passed.
- On both platforms: `cargo test --locked -p iced_core -p iced_runtime -p iced_widget --lib` — 9 passed per platform, including all four new regressions.
- Each new regression test was run against the unfixed implementation on Windows
  and failed its behavioral assertion, then passed with its upstream fix.
- A fresh checkout of the unchanged base plus the exported patch reproduces all
  34 modified vendor files in the tested checkout.
- No system settings were changed. The renderer handoff, syntax parser, and
  appearance implementations were not modified in these four backports.
