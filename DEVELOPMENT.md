# Development

See [Architecture](ARCHITECTURE.md) for module ownership, dependency boundaries,
and the contracts used by file workflows, editor commands, and UI presentation.

## Utility windows

The Windows dialog uses selectable rows to activate an application window. It
shows the current window and the number of open windows; the list scrolls in
small editor windows while its Done button remains visible.

Find and Replace uses Find and Replace tabs with an explicit Search
in selector for the current document or open documents. Switching between Find
and Replace preserves the scope. Open documents includes unsaved tabs and supports
file-name filters; it does not search unopened files on disk. Fields, options, and
actions remain visible without scrolling; grouped results use the remaining space.
The footer retains search status and errors. Opening the dialog or switching its
workflow focuses the query for immediate typing. Enter in the
query field finds the next match in the current document or lists matches across
open documents.

Go to line opens a compact prompt over the editor from Search > Go To Line or
Ctrl+G (Command+G on macOS). The current line is selected for immediate replacement;
Enter jumps and returns focus to the editor, while Escape, Cancel, or a backdrop
click dismisses without moving the caret. Invalid input keeps the prompt open.
Like the previous command, out-of-range numbers clamp to the first or last line.
The shortcut can be changed in Preferences, and the prompt preserves search state.

Preferences groups rendering and scrolling under General, color mode and syntax
under Appearance, and editing and document markers under Editor. The appearance
preview uses the draft color mode, real syntax highlighting, and draft zoom.
Select a shortcut binding to record a replacement; Cancel recording leaves the
binding unchanged. Apply, Save, and Cancel retain their existing behavior and
remain visible while a page scrolls. Switching categories resets the page scroll.

To review the actual widgets in both themes at normal and minimum content sizes:

```text
cargo run --locked --example preview_dialogs
```

The software renderer writes PNGs to `target/dialog-review/`, including populated
search results, shortcut recording, and shortcut conflicts. These previews omit
native window chrome; window-manager behavior still needs native testing.

## Window title bars

The main, Preferences, and Find and Replace windows use application-drawn title bars
on Windows, Linux, and macOS. Other targets retain system decorations. The bar
follows the light/dark theme and dims when unfocused. Long titles truncate before
the controls. Drag the caption to move a window; double-click to maximize/restore.
Windows also supports right-clicking the caption for the system window menu.
Close uses the normal session/unsaved-document flow; Settings close cancels its draft.

Windows/Linux use controls on the right. macOS uses traffic lights on the left
and a centered caption. The green control maximizes/restores (zooms), rather than
entering macOS fullscreen. Windows/Linux have custom edge and corner resize grips;
macOS retains AppKit's native resizing because winit does not support `drag_resize`
there. Native file pickers retain their system chrome.

Both styles use the transparent bunny at 24 logical pixels. Windows places it
before the caption; macOS places it in the right side slot to preserve the
centered caption and the traffic lights. The About logo keeps the rounded blue
background, with the bunny floating slightly beyond the tile and blinking once
every four seconds on a shared 60fps clock. The left paper drifts independently,
and curved, softly fading light trails lead into the decorative quill.
Opening About requests the Vulkan renderer when hardware acceleration is enabled,
using the existing prepare/warm/commit handoff and software fallback.
Late redraws retain the clock's
cadence instead of shifting each subsequent deadline. See
[Bunny artwork](assets/illustrations/bunny/README.md) for sources and regeneration.

Debug builds provide a **Window controls** switch under **About → Debug**. Clicking
it switches all application windows immediately, including windows opened later.
The switch changes appearance only; it does not emulate the other operating
system's window manager or change saved settings. Set the initial preview with:

```powershell
$env:FRAGILE_NOTEPAD_TITLE_BAR = 'macos' # or 'windows'
cargo run -- --no-session
```

Release builds omit the toggle and ignore this environment variable. Native macOS
window behavior still requires testing on a Mac; either visual style can be tested
on Windows. Title-bar widget tests exercise both styles, controls, caption dragging,
double-clicking, resize isolation, long titles, and light/dark/inactive rendering.

## Editor interactions

Whole mouse-wheel steps in settings, search results, function lists,
About tabs, menus, dropdowns, the tab strip, and the toolbar ease over 150 ms.
Repeated steps accumulate and reversing direction responds from the displayed
position, including during rapid or batched wheel input. Pixel deltas and
fractional line deltas remain direct. Windows also reports touchpads as line
deltas, so units alone cannot identify the device. A shared input heuristic
keeps a gesture direct after a fractional/pixel packet until a 250 ms pause;
event frequency alone never changes the scrolling mode. Direct input cancels
pending wheel motion at the displayed position without jumping to its target.
Touch gestures, scrollbar dragging, and keyboard reveal remain direct.
Scrolling animations retain state across widget rebuilds, clamp
to content bounds, and stop requesting frames when finished. The custom text
editor keeps its existing scrolling behavior.

The editing surface is the custom Iced `AdvancedEditor` widget. Its pointer and
keyboard handling lives under `src/editor/widget/`; commands go through
`EditorAction` and the application handlers so menus and shortcuts share undo,
clipboard, and selection behavior.

- Double-click the line-number gutter or column zero to select a complete logical
  line, including its line ending when present. Double-click elsewhere selects a
  word.
- Drag a selection beyond either vertical edge to scroll continuously. Scrolling
  speeds up with distance from the edge and stops on release, focus loss, or the
  document boundary.
- Drag inside highlighted text to move it to another position in the same
  document. The highlight stays in place until release, and a drop caret marks
  the destination. The moved text remains selected and the entire move is one
  undo step. Multiple selections and rectangular blocks insert their selected
  text together, separated by the document's line ending. A click without a
  drag places the caret normally. Escape, focus loss, or releasing outside the
  editor cancels the move; dropping onto the source leaves it unchanged.
- Collapsed code blocks show a boxed ellipsis after the header. Click the box to
  expand the represented block. The indicator follows text measurement, zoom,
  horizontal scrolling, and EOL marker spacing.
- Right-click inside a selection to retain it; clicking outside moves the caret
  before opening the menu. Multiple and rectangular selections are preserved
  when the click is inside any selected region.
- Open the editor context menu with the Context Menu key or Shift+F10. Arrow keys
  navigate commands and submenus, Enter activates, and Escape dismisses. Menus
  fit the window and support scrolling when space is limited. Text composition
  is suspended while the menu is open and resumes in the editor after dismissal.

The context menu reuses the Edit menu command definitions and configured shortcut
labels. History, clipboard, selection, line, indentation, case, search, navigation,
and folding commands use the existing handlers. Unavailable commands retain their
shortcut hints but are disabled; the toolbar shares the same availability rules.
Cut, Cut Line, and Delete Line operate on all unique touched lines in one undo
transaction, including multiple carets and rectangular selections.

Selection behavior and context-menu interactions are covered by regression tests
in `src/editor/widget/tests.rs` and `src/ui/editor_context_menu/tests.rs`.

Word Wrap reflows logical lines into screen rows at the current text width.
`ViewportModel` stores byte and visual-column boundaries for each fragment;
rendering, hit-testing, navigation, IME placement, and scrollbars share this map.
Tabs retain their logical-line stops, Unicode graphemes remain intact, and soft
breaks never change the buffer or clipboard text. Line numbers and fold controls
appear on the first fragment; end markers and collapsed ellipses appear on the
last. Resizing, zooming, and changing gutter or tab settings reflow the viewport.
Caret affinity preserves the chosen side of a soft break for End, vertical
navigation, added carets, and pointer placement. Toggling wrapping keeps a visible
caret in view. Session recovery records the logical top position so changes to
the window width do not discard the saved location. Streaming loads reflow only
the previous last line and newly appended lines. Edits that retain the logical line count
remeasure the affected lines and reuse the remaining wrap measurements; changes
to line counts or fold visibility rebuild the mapping.

## Files and sessions

Launch with file paths, including several paths at once:

```powershell
fragile-notepad.exe "notes.txt" "src/main.rs"
fragile-notepad.exe -- "-draft.txt"
fragile-notepad.exe --no-session "notes.txt"
```

The executable restores the previous session by default, then opens any paths
supplied on the command line. Already-open paths select their existing tab.
Relative paths resolve against the calling process's working directory. A second
invocation forwards its paths to the existing application and activates its window.
Forwarding succeeds only after the running application accepts the request. If
it is saving its session and exiting, the command returns an error asking you to
retry; the request is not silently acknowledged and dropped.
`--help` and `--version` exit without opening a window. `--no-session` disables
both restoration and session writes for that launch; it does not erase the previous
session. When forwarding to an existing instance, its session policy remains active.

By default, quitting preserves all tabs, including unsaved and untitled text, in
`session.json` next to `settings.xml`. Sessions retain tab order, the selected tab,
pins, encoding/line endings for recovery text, language, selection, scrolling, and
collapsed folds. Individual dirty-tab closes still use Save/Discard/Cancel.
The confirmation panel and backdrop fade in and out over 140 ms. The panel slides
up on entry and down on exit, following the same progress as its fade. Choosing an
action disables the controls and retains the modal until its fade finishes;
saving, discarding, advancing to the next prompt, and exiting wait for that fade.
Automatic language detection remains automatic after restoration. Older sessions
without this metadata infer automatic detection when the saved language matches
the file extension; explicit overrides are preserved in newly saved sessions.
Session changes are checkpointed after two seconds and flushed before quit;
a failed exit write leaves the application open. Crash recovery covers the last
completed checkpoint, not necessarily the last two seconds of editing.

Saved clean tabs reopen from disk only when selected. Missing files retain their
session entries for retry; they are never overwritten with a partial load. Unsaved
tabs retain their recovery text while deferred. Invalid or unsupported sessions
are reported and preserved rather than replaced automatically. Session storage
is bounded to 256 MiB serialized data and 10,000 tabs; exceeding either limit
reports a save failure rather than silently omitting documents.
Failed file loads remain read-only until a successful reload. Find All, Count,
and Replace All in Open Documents load their captured target tabs before running;
changing the request cancels it. If a target closes or fails to load, replacement
is canceled before changing any document.

The Recent Files menu retains the latest 16 opened/saved paths; it is separate
from the saved session and does not limit the number of restored tabs.
History/settings writes use a 250 ms debounce and an ordered latest-pending
writer. Startup merges early user changes before persisting. File reads run at
most four at once; closing a loading tab aborts its task. Fold analysis runs for
the active document on a blocking worker, and outline parsing uses at most two
blocking workers. Superseded outline tasks are aborted; a parse already running
can finish, but its stale result is rejected. Full syntax/fold/outline analysis
remains limited to documents at or below 1 MiB of decoded text.

Syntax highlighting is progressive. The editor draws immediately using available
spans. A blocking worker prioritizes visible logical lines (skipping folded blocks
and duplicate wrapped fragments), then nearby lines. These initial colors are
provisional because parsing starts without earlier document context. Interleaved
context passes parse from the beginning and replace provisional spans with exact
highlighting, including multiline comments and embedded languages.

Only one syntax batch is outstanding. Batches yield after 128 lines or about 4 ms;
a single slow line may exceed that budget on the worker. Viewport requests follow
the latest scroll position, provisional storage is bounded to 1,024 lines, and
document revision, language, theme, and cache generation checks reject stale
results. Editing invalidates parser spans but retains the last displayed colors
for up to 1,024 lines until exact replacements arrive. Retained colors follow
line insertions/deletions and Unicode text edits; they are display-only and never
count as valid parser context. Language/theme changes clear them. Syntax results
redraw the editor without triggering session writes.

The tab strip reserves space below the labels for a visible horizontal scrollbar
when the tabs overflow. When they fit, that strip disappears. Window resizing,
opening/closing tabs, and title changes update this decision during layout.

Configuration and cache locations are defined in [src/platform/paths.rs](src/platform/paths.rs):

| Platform | Configuration (`settings.xml`, `session.json`) | Cache |
| --- | --- | --- |
| Windows | `%APPDATA%/FragileNotepad` | `%LOCALAPPDATA%/FragileNotepad/Cache`, falling back to `%APPDATA%/FragileNotepad/Cache` |
| Linux/macOS | `$XDG_CONFIG_HOME/fragile-notepad`, otherwise `$HOME/.config/fragile-notepad` | `$XDG_CACHE_HOME/fragile-notepad`, otherwise `$HOME/.cache/fragile-notepad` |

On Unix, session snapshots are written with owner-only file permissions. Clean
file tabs store paths rather than copying disk contents; unsaved recovery text is
stored in the session file. The raw small-file cache service is not populated by
normal open/save operations.

## Bulk-load and recovery profiling

The following uses an isolated settings directory and synthetic files. Never point
the profiler at your normal application configuration:

```powershell
$env:APPDATA="$PWD/target/many-files-investigation/profile"
$env:LOCALAPPDATA=$env:APPDATA
cargo run --release --no-default-features --example profile_many_files -- 100 rust64
cargo run --release --no-default-features --example profile_many_files -- history
cargo run --release --no-default-features --example profile_many_files -- outline
cargo run --release --no-default-features --example profile_many_files -- recovery
cargo run --release --no-default-features --example profile_many_files -- recovery verify
```

On Linux/macOS, set an absolute isolated `XDG_CONFIG_HOME` containing a
`many-files-investigation` directory component instead of `APPDATA`, for example:

```bash
export XDG_CONFIG_HOME="$PWD/target/many-files-investigation/profile"
export XDG_CACHE_HOME="$PWD/target/many-files-investigation/cache"
cargo run --release --no-default-features --example profile_many_files -- 100 rust64
```

Run these in a dedicated shell or restore the previous environment values when
finished. `profile_many_files` is a diagnostic harness and closes its window
automatically; use the application binary for an interactive inspection:

```powershell
cargo build --release --locked
$files = Get-ChildItem "$env:APPDATA/fixtures/document_*.rs" | Sort-Object Name | Select-Object -First 50
& .\target\release\fragile-notepad.exe --no-session @($files.FullName)
```

Available workloads are `text4` (4,080 bytes/file), `text64` (68,000), `rust64`
(66,600), `rust900` (962,000), and `rust1100` (1,221,000). `model` measures handlers
and widget construction without running the returned asynchronous tasks; `history`
measures settings parsing/writes; `outline` measures declaration scaling. Neither
`model` nor first-view timing alone measures all-files-ready latency.

The recovery pair verifies 101 tabs and exact unsaved Unicode text across two real
window lifecycles. Bulk-load output separates file-completion handler time,
all-files-loaded time, persistence results, and a 16 ms UI heartbeat. Startup tests
report first-view entry and a separate screenshot-completion milestone after
layout/rendering; screenshot completion is not an OS presentation timestamp.
The debug startup test allows 200 ms on local native systems, 750 ms on hosted
Windows/Linux runners, and 2 s on hosted macOS runners. These are wall-clock measurements that
include native window setup. The first-view and first-frame probes each retain
a five-second timeout, and test diagnostics report both timings and the budget.

## Vendored Dependencies

See [vendor/README.md](vendor/README.md) for upstream revisions, licenses, and
local changes.

For upstream updates, compare the recorded and desired revisions, port the
selected changes into `vendor/`, and update the provenance notes. Run application
checks and affected vendor tests with the application's lockfile, for example
`cargo test --locked -p iced_wgpu --lib`.

## Validation

Generated raw RGBA icon files are not tracked. Rebuild them after changing SVG
icon sources (the colored family now uses SVGs too):

```powershell
.\scripts\generate_icon_assets.ps1
```

On Linux or macOS:

```bash
bash scripts/generate_icon_assets.sh
```

Icon design, online references, and licensing are documented in
[`assets/icons/README.md`](assets/icons/README.md). After regeneration,
`python scripts/preview_icons.py` builds an offline light/dark gallery at
`target/icon-review/index.html` with native-size raster/vector comparisons.

The CI entry points run the standard local validation sequence without
formatting vendored path dependencies. They call the icon generation script
before compiling:

```powershell
.\scripts\ci.ps1
```

On Linux or macOS:

```bash
bash scripts/ci.sh
```

The GitHub Linux jobs install `libvulkan1`, `mesa-vulkan-drivers`, `vulkan-tools`,
`xvfb`, and `xauth` in addition to the windowing libraries. Before validation,
`scripts/setup-ci-vulkan.sh` selects Mesa Lavapipe, exports `WGPU_BACKEND=vulkan`
and the Vulkan ICD variables for later steps, and verifies adapter enumeration.
The parity test remains mandatory; this uses the wgpu pipeline with a software
Vulkan adapter rather than opting out. A local Linux run needs an available
Vulkan driver too; Xvfb supplies an X11 display, not a Vulkan adapter.

Before handing off changes that touch editor rendering or vendored source, run:

```powershell
cargo check
cargo test
cargo check --examples
cargo check --no-default-features
```

## Packaging

See [PACKAGING.md](PACKAGING.md) for release builds.

For renderer performance changes, also run:

```powershell
cargo run --release --no-default-features --example profile_tiny_skia_text
cargo run --release --example profile_render
```

For the patched software backend hot paths, run without tracing so per-event
diagnostics do not affect the measurements:

```powershell
cargo run --release --no-default-features --example profile_backend_hotspots
cargo run --release --no-default-features --example profile_tiny_skia_text
```

`profile_backend_hotspots` measures the production scroll detector, cached image
filtering, and damage compaction. Its nearest-filter numbers are a comparison;
the optimization preserves the original linear-filter output. Pixel-equivalence
regressions live in the vendored graphics and tiny-skia crates:

```powershell
cargo test --locked -p iced_graphics --lib
cargo test --locked -p iced_tiny_skia --lib --features iced_tiny_skia/image
cargo test --locked -p iced_winit -p iced_wgpu --lib
```

The application icon parity test also checks cached-frame equality at 100%,
150%, and 200% scale. Its top-edge check allows a one-level alpha rounding
difference at the same pixel; extra rows with greater coverage differences still
fail. Pixel-difference limits remain separate from that edge check.

## Backend Switch Probe

The backend switch probe verifies the runtime path for starting with the
software renderer, requesting the prepare/warm/commit backend handoff to
`Backend::Hardware(Api::Vulkan)`, running real wgpu offscreen warm-up, observing
strict frame-order evidence, and exiting. Use trace collection for strict
validation; without it, the probe may report `indeterminate` because strict
success requires warm-up and present-order evidence:

```powershell
$env:CARGO_TARGET_DIR='target-codex-check'
$env:FRAGILE_PERF_TRACE='1'
cargo run --example backend_switch_probe -- --scenario=single-window
```

The handoff submits the current UI offscreen for each window, polls GPU completion
without waiting on the event loop, and retains those renderers for commit. The
warm-up deadline is three seconds. Visible surface creation/configuration still
runs at commit, so successful frame-order evidence is not a zero-stall guarantee.
See [the rendering architecture](SEAMLESS_HYBRID_RENDERING.md) for the state flow,
cache boundaries, and remaining platform-validation limits.

On Windows, keep the generated result JSON and trace CSV from the run. Current
Windows strict validation records `result=ok`, strict outcome success,
Wgpu/Vulkan presented evidence, and non-null warm evidence for both
single-window and multi-window scenarios. For WSL/Linux, all nine strict
scenarios pass using Mesa Lavapipe under WSLg, including the actual About
animation. This proves software Vulkan correctness; physical-GPU Linux
presentation is still unverified. Native macOS has not been locally validated;
CI configures MoltenVK and the Vulkan loader as described in
[PACKAGING.md](PACKAGING.md).

To measure sustained redraw cadence and CPU frame costs after a successful
strict handoff, build the release probe once, then run workloads sequentially:

```text
cargo build --locked --release --example backend_switch_probe
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe --workload editor
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe --workload editor --windows 2
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe --workload plain-text
python scripts/test_profile_vulkan_live.py
```

Omit `.exe` on Linux/macOS and configure the platform Vulkan runtime first.
Keep the probe visible and focused: About intentionally pauses when unfocused,
so its sustained workload uses one window. Editor workloads advance three rows
per 24 Hz timer tick. The probe removes its frame subscription and changing
status labels during measurement; About retains its own animation scheduler.
Each run defaults to 12 seconds after handoff, discards the first two seconds,
and saves its trace, log, strict result, and summary under `target/vulkan-live/`.
Use `--seconds` and `--warmup-seconds` to change the interval. The runner rejects
missing presentation evidence, software fallback, and insufficient samples.
These instrumented redraw wall times include interaction, drawing, tracing,
and presentation waits, but exclude separate application updates/UI rebuilding;
they do not measure GPU execution or display scanout latency. Initial prepare
and commit delays belong to the handoff scenario and are outside this interval.

Linux CI also runs the release probe on an isolated Weston Wayland compositor
with a private socket and runtime directory:

```text
cargo build --locked --release --example backend_switch_probe
python scripts/check-wayland-vulkan.py --binary target/release/examples/backend_switch_probe
```

Install Weston and configure Vulkan first (CI uses Lavapipe). The runner defaults
to a headless Pixman compositor, checks all handoff scenarios plus sustained
About/editor workloads, and saves logs/traces under `target/vulkan-wayland/`.
Use `--backend=x11` for a visible nested compositor. `--weston-root` supports
locally extracted Debian amd64 Weston packages without system installation.
The runner terminates its compositor and removes its private runtime directory
afterward. Headless or nested software Vulkan does not prove physical-GPU Linux
presentation. Local WSLg's older Weston crashes with signal 11 in some release
captures; Weston 14.0.2 passes the strengthened checks. See `VULKAN_RENDERING.md`
for the retained crash logs and limits of that comparison.

For diagnostics against the older basic configure task, run the probe in
configure mode:

```powershell
cargo run --example backend_switch_probe -- --mode=configure
```

or set `FRAGILE_BACKEND_SWITCH_PROBE_MODE=configure`.

The probe also supports lifecycle scenarios with `--scenario=` or
`FRAGILE_BACKEND_SWITCH_PROBE_SCENARIO`:

```powershell
cargo run --example backend_switch_probe -- --scenario=single-window
cargo run --example backend_switch_probe -- --scenario=multi-window
cargo run --example backend_switch_probe -- --scenario=resize-during-preparing
cargo run --example backend_switch_probe -- --scenario=close-during-preparing
cargo run --example backend_switch_probe -- --scenario=close-during-commit-pending
```

`single-window` is the default strict scenario and reports `result=ok` on
current Windows validation when trace evidence proves warm-up completion and
frame ordering. `multi-window` opens an additional window before switching and
requires a post-switch frame from every live window; current Windows validation
reports `result=ok` with Wgpu/Vulkan presented evidence for both live windows
and non-null warm evidence. The resize and close scenarios set
`FRAGILE_NOTEPAD_RENDER_PREPARE_DELAY_MS=250` when no delay is already present
so the requested lifecycle operation has a chance to occur during preparation.
`close-during-preparing` reports `result=ok` for the intentional
`Cancelled + Preparing + NotNeeded` cancellation path.
`close-during-commit-pending` enables trace collection when needed, waits for
the `backend_handoff_commit_pending` phase marker, and uses the probe-only
`FRAGILE_NOTEPAD_RENDER_COMMIT_PENDING_DELAY_MS=250` diagnostic hook when no
delay is already present so the close request can be processed before commit.
If trace evidence cannot prove exact timing, it reports `indeterminate` instead
of silently passing.

Runtime rendering policy can be forced without changing saved settings:

```powershell
$env:FRAGILE_NOTEPAD_RENDER_BACKEND='software'              # force software
$env:FRAGILE_NOTEPAD_RENDER_BACKEND='lazy-gpu'              # request lazy boost
$env:FRAGILE_NOTEPAD_RENDER_BACKEND='hardware-diagnostic'   # diagnostic boost
```

Saved settings use the same policy with `hardware_acceleration` modes exposed in
Settings: software, lazy hardware, and hardware diagnostic. The environment
override wins over saved settings for startup and runtime boost decisions.

Hybrid rendering is enabled by default. With `--no-default-features`, the
example still compiles and prints a skip marker explaining that the feature is
required. Stable CLI markers include
`BACKEND_SWITCH_PROBE_START`, `BACKEND_SWITCH_PROBE_INITIAL_FRAME`,
`BACKEND_SWITCH_PROBE_HANDOFF_COMMIT_REQUESTED`,
`BACKEND_SWITCH_PROBE_POST_SWITCH_FRAME`, `BACKEND_SWITCH_PROBE_DONE`, and
timeout or switch-failure markers when the probe cannot complete. Configure
mode additionally emits `BACKEND_SWITCH_PROBE_CONFIGURED`. Scenario runs emit
`BACKEND_SWITCH_PROBE_SCENARIO_*` markers, and every completed run attempts to
write a JSON result log before exit:

```text
target/hybrid-rendering-probes/<mode>-<scenario>-<failure>.json
```

Set `FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR` to write these logs elsewhere.
Each JSON result includes the mode, scenario, failure injection, OS, arch,
result (`ok`, `failed`, `skipped`, or `indeterminate`), reason, state,
`strict_outcome`, frame count, window open/close counts, resize observations,
trace path when used, and trace-derived timing or renderer evidence when
available. Strict trace evidence includes:

- `warm_complete_us`
- `warm_elapsed_us`
- `warm_renderer_family`
- `warm_backend`
- `warm_adapter`
- `warm_passes`
- `warm_submission_completed`
- `warm_timeout_ms`
- `warm_failure`

For a strict success run, `trace_evidence.warm_complete_us` must be present,
`warm_renderer_family` must be `Wgpu`, `warm_submission_completed` must be
`true`, warm completion must occur before commit pending, software frame
evidence must be present during prepare and commit-pending, and the first
post-commit backend must be hardware. Missing timing or trace proof is
`indeterminate`; wrong renderer family or failed warm-up submission is `failed`.

Failure injection is opt-in and inactive by default. To exercise failure
handling in the prepare/warm/commit path:

```powershell
cargo run --example backend_switch_probe -- --fail=prepare
cargo run --example backend_switch_probe -- --fail=warm
cargo run --example backend_switch_probe -- --fail=commit
cargo run --example backend_switch_probe -- --fail=first-present
```

The probe prints `BACKEND_SWITCH_PROBE_INJECTED_FAILURE_OBSERVED` when the
requested failure reaches the app boundary. Equivalent environment switches are
`FRAGILE_BACKEND_SWITCH_PROBE_FAILURE` for the probe and
`FRAGILE_NOTEPAD_RENDER_INJECT_FAILURE=prepare|warm|commit|first-present` for
the runtime. `FRAGILE_NOTEPAD_RENDER_PREPARE_DELAY_MS=<milliseconds>` can delay
the prepare phase for diagnostics.
`FRAGILE_NOTEPAD_RENDER_COMMIT_PENDING_DELAY_MS=<milliseconds>` can delay the
commit-pending phase for the backend switch lifecycle probe; leave it unset
outside diagnostics.

Set `FRAGILE_PERF_TRACE=1` to write the renderer trace CSV, and optionally set
`FRAGILE_PERF_TRACE_DIR` to choose the output directory. When trace collection
is enabled and no explicit trace directory is provided, `backend_switch_probe`
uses `CARGO_TARGET_DIR/perf/<scenario>/<mode>-<failure>/fragile-perf.csv`.
Application and renderer loggers append to the shared trace; use a fresh
directory or remove the old trace before launching a new capture. The probe
resets its trace before starting Iced, and profiling runners use fresh directories.
The CSV should include the Phase 1 trace markers `fallback_present_start`,
`fallback_present`, and the existing `winit_redraw_frame` event. In this repo's
fallback ordering, `Primary` maps to wgpu and `Secondary` maps to tiny-skia, so
trace backend identity is available through the `backend=wgpu` or
`backend=tiny-skia` fields.
Strict warm-up evidence is emitted as `backend_handoff_warm_complete` on
success or `backend_handoff_warm_failed` on failure. The detail fields include
the warm-up elapsed time, renderer family, adapter, backend, pass count,
submission completion, timeout, and failure text when applicable.

In prepare/warm/commit mode, `result=ok` is strict evidence for the selected
scenario only; it is not a release claim by itself. Current Windows
single-window and multi-window strict probes pass with warm-up and first GPU
frame evidence. The lifecycle scenarios add structured evidence for local runs,
but they are not a substitute for the full platform matrix in
`SEAMLESS_HYBRID_RENDERING.md`. Cargo commands may emit vendored `encoding_rs`
lifetime syntax warnings.
