# Iced provenance and local customizations

Reviewed 2026-09-21 against upstream master
`7abcb02ca81a0d33bfbaf9d37e5858a5d440cd8f` (320 commits after the previous pin).

The local source derives from `ddd7c42a9ba625b219e5e8062ff9be83eea467c5` (2026-06-25),
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

The local source also backports recent fixes without the intervening API migration:

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

Upstream: https://github.com/iced-rs/iced

License: [MIT](LICENSE).

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

The GPU probes test handoff success and rollback. Normal driver surface-error
recovery is an upstream backport; a real driver surface failure was not induced.

## Follow-up input/overlay backport validation

- Windows: `cargo test --locked --all-targets` — 686 passed.
- Debian WSL 2: the same command — 695 passed.
- On both platforms: `cargo test --locked -p iced_core -p iced_runtime -p iced_widget --lib` — 9 passed per platform, including all four new regressions.
- Each new regression test was run against the unfixed implementation on Windows
  and failed its behavioral assertion, then passed with its upstream fix.

## Application image atlas sizing

The Vulkan animation resource profile found a 16 MiB image atlas per window.
The application fork now starts with a 1024-square page (about 4 MiB); the full
About float/blink cycle grows to 8 MiB. Existing layer growth and fragmentation
still support larger image sets. GPU timing at 100%, 150%, and 200% scaling was
unchanged within measurement noise. See `VULKAN_RENDERING.md` for measurements
and platform-validation limits. This is an application-specific default.

## Optional immediate parameters

The window compositor and headless constructor request wgpu `IMMEDIATES` when
supported, with the device limit bounded to 128 bytes. The About trail uses
16-byte Vulkan push constants to avoid per-widget uniform allocations and queue
writes. Its uniform path remains available for devices created without this
optional feature. GPU creation still happens after software startup; the
prepare/warm/commit/first-present contract is unchanged.

Quad transforms now use 80-byte immediate parameters and image transforms use
64 bytes when the requested device supports those sizes. Smaller requested
limits retain the uniform path independently for each pipeline. Images share
two engine-level sampler bindings instead of creating them for every layer.
The two-window, six-layer pixel regression covers limits 0/16/64/80, scaling,
clipping, gradients, both image filters, changing transforms, and teardown.
It passes on NVIDIA, AMD, SwiftShader, and Linux Lavapipe. In that fixture,
buffers fall from 30 to 18 and bind groups from 23 to 7 at the full limit.
The measured editor trace eliminates 553 transform-buffer copies over 144
steady frames; moving glyph/instance uploads remain unchanged.

## Demand-driven GPU allocation

Quad layers allocate solid/gradient buffers only when each type first appears,
sized to the actual batch and retaining the existing growth/reuse behavior.
This removes the fixed 2,000-instance allocation for unused types and saves
roughly 767 KiB per About window and 2.23 MiB per profiled editor window.

Triangle and optional MSAA pipelines are initialized on first visible mesh
preparation, shared through `Arc<OnceLock<_>>` across engine clones. Per-renderer
mesh state is also deferred. Meshes used by a handoff scene still initialize
during offscreen warm-up. Later first use incurs compilation during preparation.
The application regression `tests/vulkan_resource_reuse.rs` covers buffer growth,
empty frames, layered/multi-window rendering, lazy mesh use, MSAA, and resizing.

Image caches/workers are created on first image use. Atlas textures/bindings are
created only on upload, so measurement and standalone worker uploads do not
allocate an unused shared page. The first upload creates all needed layers;
subsequent growth preserves live pixels. Image and gradient-quad pipelines also
initialize on demand, shared across engine clones. The resource regression covers
measurement, duplicate allocation callbacks, allocation leases, atlas growth,
ordinary asynchronous uploads, pipeline sharing, and teardown.

Small raster images (up to 128 pixels per side) use a separate lazy 256-square
atlas, while larger artwork and SVGs keep the 1024-square atlas. Cache entries
explicitly identify the owning pool or dedicated worker binding, including
during eviction. The profiled Rust editor saves 3.75 MiB per window with its fold
icons; the measured About scene is unchanged. Mixed scenes can use both pools.
The resource regression covers repeated small-image growth/eviction alongside
retained large images and independent worker uploads.

Image uploads larger than the 100 KiB pooled-upload limit use a temporary mapped
buffer. The command submission retains it until GPU completion, then releases
it. Small uploads remain pooled. This saves 2.73 MiB of retained buffer memory
per measured About window. The atlas regression checks delayed submission,
padding, fragmentation, atlas growth, and release after completion.

Renderer and image-cache staging belts now start with 4 KiB chunks and grow to
fit actual writes, replacing the fixed 100 KiB, 2 MiB, and 4 MiB defaults.
Cryoglyph is maintained in `../cryoglyph` for glyph upload/lifetime control.

Quad and image instance buffers retain a CPU copy of their uploaded bytes.
Unchanged draws skip the copy; changed draws upload one aligned span excluding
unchanged leading/trailing bytes. Buffer growth invalidates the retained copy.
Pixel regressions compare reused renderers with fresh renderers across repeated,
changed, and reverted frames, both parameter paths, and fractional scaling.

Atlas growth now respects requested device layer/dimension limits. Fragmented
allocation is transactional: failure frees its reservations before upload.
Full icon atlases spill into the main pool; full shared pools use independent
textures when possible. Images that cannot fit report an allocation error,
including completion/wakeup for asynchronous worker callbacks. A real-Vulkan
regression requests a two-layer device, verifies spill/reuse and retained pixels,
and reproduces the original invalid-texture error before the fix. This does not
simulate physical-VRAM exhaustion.

## Synchronous window resizing

The runtime now handles the physical size returned by `request_inner_size`.
Winit's Wayland backend applies this request synchronously and may emit no later
resize event. Previously Iced retained the old viewport/surface size and warmed
Vulkan at that stale size. The returned size now updates window state, queues the
logical resize event, and requests redraw; normal redraw processing relayouts and
reconfigures the surface. The returned size is authoritative when the window
system clamps or rejects a request.

The strengthened handoff probe fails before this fix on Wayland (640 x 380 after
requesting 700 x 440) and passes after it. The matrix runner also requires a
successful software presentation at the requested logical size before Vulkan
warm-up, with matching physical warm-up dimensions at the current scale.

## Fractional image motion

Raster images expose `Image::snap(false)` to retain fractional physical bounds
and clipping during animation. Snapping remains enabled by default for existing
widgets. Both tiny-skia and wgpu honor the option; the software path bypasses
the integer-position resample cache and uses bilinear image transforms.
The About bunny, paper, and background opt out of snapping. Pixel regressions
exercise consecutive animation frames at 100%, 150%, and 200% scaling on both
renderers, alongside the application's 60 Hz scheduling regression.

Windows validation: all 732 application/example tests and 24 tiny-skia tests
pass, as does the software-only build check. An eight-second live Vulkan About
probe on an NVIDIA RTX 5070 Laptop GPU at 150% scaling passed strict handoff
and measured 60.0 fps after warm-up (16.7 ms median, 17.8 ms maximum frame
interval). This measures redraw/presentation cadence, not display scanout.
