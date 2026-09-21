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

`BASE_REVISION` and `fragile-notepad-iced.patch` are the reproducible source of the
vendor checkout. Run `scripts/setup-vendor.ps1 apply` (Windows) or
`scripts/setup-vendor.sh apply` (Linux) to reconstruct it in a fresh checkout.

## Validation

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
