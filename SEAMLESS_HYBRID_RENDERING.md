# Hybrid Rendering

This document describes the current implementation. It replaces the earlier
design plan and its illustrative API sketches. Setup, profiling commands, and
the full probe invocation list are in [DEVELOPMENT.md](DEVELOPMENT.md).

## Startup and policy

The application always starts with `Backend::Software` (tiny-skia),
antialiasing disabled, and vsync disabled, as configured by
[src/startup.rs](src/startup.rs). The default Cargo feature `hybrid-rendering`
adds `iced/wgpu`; `--no-default-features` keeps the software renderer.

[src/app/rendering.rs](src/app/rendering.rs) resolves the rendering policy from
saved settings and the optional `FRAGILE_NOTEPAD_RENDER_BACKEND` override:

| Override | Policy |
| --- | --- |
| `software` | Suppress hardware-boost requests |
| `lazy-gpu` | Permit the normal hardware handoff |
| `hardware-diagnostic` | Permit hardware handoff for diagnostics |

A recognized environment value takes precedence over saved settings. Invalid
values are ignored and identified in the About debug information. This policy
controls boost requests; changing it to software does not switch an already-active
GPU renderer back to tiny-skia.

When saved settings load, a lazy/diagnostic policy requests a boost once the main
window is open. With no saved settings, that load-time branch does not request a
boost. Opening About can request a permitted boost. The app starts the About
animation only after the strict handoff succeeds; failures leave software usable.
Find, inline replace, and function-list visibility also have app-managed animations.

The app states are `Software`, `PreparingHardware`, `Hardware`, and
`Failed(RenderFailureCategory)`. Duplicate requests are suppressed while preparing
or already using hardware. A failure suppresses further attempts for that process;
there is no timed retry loop.

## Runtime handoff

The app calls `backend::prepare_warm_and_commit` and receives
`backend::StrictHandoffOutcome` through `Message::BackendBoostConfigured`.
The older `backend::configure` task remains available for diagnostics, but is not
the production strict-handoff path.

The implementation lives in the exported
[iced patch](patches/iced/fragile-notepad-iced.patch), primarily
`winit/src/lib.rs`, `wgpu/src/lib.rs`, `graphics/src/compositor.rs`, and
`renderer/src/fallback.rs` inside the vendor checkout.

1. **Prepare:** create the pending GPU compositor asynchronously while the window
   manager retains the software compositor, renderers, and surfaces. Completion
   wakes the runtime; preparation does not continuously request redraws just to
   poll its future.
2. **Warm:** create a pending renderer for each live window and draw its current
   UI into that renderer. `begin_warm_up_offscreen` submits the recorded primitives
   to an offscreen texture. `poll_warm_up_offscreen` polls completion without
   waiting on the event loop; pending polls schedule a redraw deadline about
   16 ms later. The GPU warm-up deadline is three seconds.
3. **Commit pending:** retain the warmed compositor and renderers. Keep software
   active until the commit boundary, following a successful software presentation.
4. **Commit:** install the warmed renderers and create/configure their visible
   surfaces. Retain the old compositor and per-window rendering state for rollback.
   Request the hardware frame immediately.
5. **Await first presentation:** require evidence that each required live window
   presented through wgpu. Missing presentation has its own three-second timeout.
   On success, release the retained software resources and report completion.
   On failure, restore the retained renderer state and report the failure/rollback
   outcome.

The synchronous `warm_up_offscreen` compatibility method still exists, with a
bounded wait. The strict runtime path uses the begin/poll methods instead.

## Visual continuity and resource ownership

Software can continue producing frames during preparation and warming. The strict
frame boundary is commit: the final successful pre-commit frame is software, and
the first successful post-commit frame must be hardware. This does not mean that
all software frames stop when GPU preparation starts.

The warmed renderers survive into commit, including their renderer-local image
and text state. Device/pipeline resources shared by the GPU engine are also reused.
The temporary renderer is no longer empty or discarded before the first real UI
frame.

Graphics geometry caches can contain backend-specific values. The runtime
invalidates them around the temporary GPU draw, before returning to software,
and at commit/rollback. This avoids mixing fallback renderer families while
retaining renderer-local warm resources.

Visible surface creation/configuration still happens synchronously at commit.
Driver and OS work can therefore stall that boundary. The implementation does
not promise zero-millisecond switching or prove that every asynchronously loaded
image is ready merely because a warm submission completed.

Failure categories distinguish prepare, warm-up, commit, first presentation,
cancellation, unsupported operation, missing renderer evidence, and rollback
problems. Window closing/resizing and injected failures are covered by the probe
scenarios below. A successful probe is evidence for that scenario and machine,
not a universal no-flash guarantee.

## Software rendering optimizations

The patch retains the existing filtering, clipping, and blending rules while
reducing repeated work:

| Path | Current behavior |
| --- | --- |
| Text scroll matching | Skip identical scenes, prune candidates that cannot beat the best match, and limit matching to 65,536 comparisons. Fall back to ordinary damage redraw when the budget is exhausted. Compute each layer's candidate once per presentation. |
| Fragmented damage | Cache the best merge for each row of the pair matrix while preserving the greedy merge order. Above 256 regions, conservatively redraw their bounding union. |
| Frame retention | Share layer snapshots through `Arc<[Layer]>` on zero-damage frames. Changed frames still snapshot their layers. |
| Scroll copy | Copy opaque, fully bounded scroll regions in place. Translucent or clipped cases keep the snapshot/composition path. |
| Linear images | Cache the exact straight-alpha resampling result by image identity and physical size. The cache is limited to 128 entries and 4 Mi pixels (16 MiB of RGBA); oversized results are transient. |
| Text clipping | Borrow contiguous full-width row strips; reuse scratch storage for narrower crops. |
| Clip masks | Reuse identical bounds and clear the previous rectangle's area between changes, using the same rasterizer for fractional coverage. |

These reduce specific costs; they do not make every frame allocation-free.
Paragraph misses can still rasterize full paragraphs, changed frames still clone
layer data, and native presentation costs remain platform-dependent.

## Diagnostics

Enable `FRAGILE_PERF_TRACE=1` to collect CSV events and optionally set
`FRAGILE_PERF_TRACE_DIR`. App, winit, fallback compositor, and tiny-skia events
share the trace file. Primitive-level software events are buffered until draw or
presentation boundaries; strict handoff phase evidence is flushed for probe
consumption. Tracing still formats records and performs I/O, so use untraced
release benchmarks for representative timings.

Useful timing fields in `tiny_skia_present` include `scroll_us`,
`damage_us`, `snapshot_us`, `grouping_us`, and `os_present_us`.
`tiny_skia_present_draw` reports damage regions, layer/primitive counts,
clip-mask reuse/rebuilds, paragraph-raster hits/misses/bypasses, and glyph counts.
These fields cover different scopes and should not be treated as interchangeable
whole-frame timings.

For strict handoff evidence, retain both the probe JSON and its CSV. Warm evidence
includes renderer family, backend/adapter, submission completion, dimensions, pass
count, and elapsed time. Commit and first-present events establish ordering.
The JSON's `warm_timeout_ms` field is error evidence, not an assertion that every
successful run reports its configured timeout.

## Validation

Run from the application repository root:

```powershell
cargo check --locked
cargo test --locked
cargo check --locked --examples
cargo check --locked --no-default-features
cargo test --manifest-path vendor/iced/Cargo.toml -p iced_graphics --lib
cargo test --manifest-path vendor/iced/Cargo.toml -p iced_tiny_skia --lib --features image
cargo test --manifest-path vendor/iced/Cargo.toml -p iced_winit -p iced_wgpu --lib
$env:FRAGILE_PERF_TRACE='1'
cargo run --example backend_switch_probe -- --scenario=single-window
cargo run --example backend_switch_probe -- --scenario=multi-window
cargo run --example backend_switch_probe -- --scenario=resize-during-preparing
cargo run --example backend_switch_probe -- --scenario=close-during-preparing
cargo run --example backend_switch_probe -- --scenario=close-during-commit-pending
cargo run --example backend_switch_probe -- --fail=prepare
cargo run --example backend_switch_probe -- --fail=warm
cargo run --example backend_switch_probe -- --fail=commit
cargo run --example backend_switch_probe -- --fail=first-present
```

The close/failure scenarios can pass by proving the expected cancellation or
rollback; they are not expected to produce a normal successful switch.
Without trace evidence, strict runs may be `indeterminate`.
A software-only build of the probe prints a feature-required skip marker.

Recorded local Windows validation passed these nine scenarios, including rollback
after first-present failure. Icon parity checks compare CPU/GPU output at 100%,
150%, and 200% scale and require cached-frame equality within each renderer.
Vendor tests compare optimized pixel-copy, clipping, resampling, mask, and damage
behavior against reference paths.

Windows results do not establish Linux/macOS rendering behavior. Previously
recorded WSL/Linux strict runs were blocked by GPU adapter creation; native macOS
strict handoff remains unverified. Full IME interaction and manual no-flash checks
across supported drivers/platforms remain separate validation work.

## Reproducing the vendor changes

Export changes from `vendor/iced` using the patch workflow in
[DEVELOPMENT.md](DEVELOPMENT.md#patch-workflow). Commit the patch and base revision
together. A dirty vendor checkout is expected after application of the patch.
Do not infer reproducibility from source timestamps alone: review vendor status,
validate patch application, and run the relevant package tests.
