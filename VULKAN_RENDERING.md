# Vulkan rendering work

Objective: unified Vulkan across Windows, Linux and macOS; explicit GPU resource
ownership and measured optimization, retaining software-first startup and the
prepare/warm/commit/first-present rollback contract. This work is incomplete.

## Status (2026-09-22)

Implementation and basic validation are complete; deep optimization is paused.
Transform pixel/resource tests pass on NVIDIA, AMD, SwiftShader, and Linux
Lavapipe. About/editor offscreen scenarios pass. Windows local CI passes build,
formatting, example, and software-only checks, plus 725 Rust and 12 Python tests.
Log: `target/vulkan-transform-windows-ci.log`.

Native macOS, physical-GPU Linux, extended live rendering, and power/throughput
validation remain deferred. Live handoff results below predate the transform
change.

## Completion requirements

- A Vulkan-only hardware feature graph, including portability support on macOS.
- Reproducible Vulkan loader/MoltenVK distribution and CI on all three platforms.
- GPU-resident animation with bounded, reused pipelines, buffers and bindings;
  equivalent software visuals during startup, handoff and fallback.
- Measurements of CPU frame recording, GPU work, uploads, memory and resource
  churn for animation, editor scrolling, resizing and multiple windows.
- Evidence-driven improvements to renderer allocations, submissions, caches and
  lifetimes, with before/after measurements and correctness regressions.
- Successful startup, warm-up, presentation, resizing, closing, cancellation and
  rollback validation on Windows, Linux and macOS. Headless rendering or software
  Vulkan alone is not proof of physical-GPU presentation on a platform.
- Reproducible, checked-in vendor source and passing application/vendor checks.

## Initial evidence

- Application commit: `7bb1ec8`. Working tree was clean before this work.
- All application hardware requests select Vulkan, but `iced/wgpu` still enables
  default Metal/DX12/GL/WebGPU features. macOS Vulkan portability is not enabled.
- About's trail computes a fresh CPU raster and image handle every animation
  update. Iced uploads that image into its atlas; the previous image is evicted.
- Iced supports application-owned shader pipelines inside its existing render
  pass. Pipeline storage is shared with the GPU engine and survives warm-up.
- Windows strict handoff previously reported Vulkan on RTX 5070 Laptop GPU.
- Debian WSL is available. Current Vulkan runtime availability needs checking.
- Native macOS evidence is missing; existing CI/package configuration has no
  MoltenVK preparation or distribution.

## Implemented and measured (2026-09-22)

The hardware feature graph now uses `iced/wgpu-bare` plus explicit wgpu Vulkan
and portability features. Software-only builds remain supported. No Vulkan
instance, pipeline, or uniform buffer is constructed during software startup.
The prepare/warm/commit/first-present handoff protocol is unchanged.

The About trail is now a procedural WGSL primitive in Iced's existing render
pass. It owns one pipeline per engine and supplies 16 bytes through Vulkan push
constants, with no per-widget GPU buffers or bindings. Devices created without
wgpu's optional `IMMEDIATES` capability retain a reusable uniform/binding per
widget, updating only when parameters change. It needs no trail texture, vertex
buffer, additional render pass, submission, or readback. The prior CPU trail was
up to 140 x 48 x 4 = 26,880 bytes of pixel payload per animation update (before
upload alignment); the shader sends 16 parameter bytes. Static artwork still
uses the image atlas. Software rendering and rollback retain the CPU field.

In the uniform fallback, widget and recorded-frame ownership, rather than a
frame-age heuristic, retains resources. Engine trimming releases slots once both owners disappear;
an idle window keeps its resources while other windows render. Actual Vulkan
resource reports verify two buffers/two bindings/one pipeline for two instances,
no textures, and release on teardown. A separate push-constant regression prepares
240 instances with one pipeline and no added buffers, bindings, or textures.
Rendered tests cover both parameter paths, phases, light/dark, opacity, clipping,
scales 1/1.5/2, and CPU → GPU → CPU phase preservation.

The compositor and headless renderer request supported immediate parameters
with a limit bounded to 128 bytes. Device creation remains lazy, after software
startup. The trail checks both the device feature and its 16-byte limit before
selecting this path.

### Upload trace

`gpu-profiling` enables wgpu API tracing and counters for diagnostic builds.
Two 240-frame, single-window captures compare uniforms and push constants.
After the first 96 frames (one animation cycle), the remaining 144 frames had:

| GPU transfer | Uniform path | Push-constant path |
| --- | --- | --- |
| Trail queue writes | 144 / 2,304 bytes | 0 |
| Quad uniform copies | 11,520 bytes | 11,520 bytes |
| Solid quad copies | 115,200 bytes | 115,200 bytes |
| Image uniform copies | 9,216 bytes | 9,216 bytes |
| Image instance copies | 48,384 bytes | 48,384 bytes |
| Glyph vertex copies | 951,552 bytes | 951,552 bytes |
| Image atlas uploads | 0 | 0 |

Both traces additionally copy 2,304 timestamp bytes for the profiler. Mapped
staging writes cover approximately 108.6 MB of host ranges; they are not GPU
transfer volume. Blink images finish uploading during the first cycle, so the
trace does not support adding a separate image-retention API. Glyph vertices
dominate recurring transfer volume in this scene.

Captures: `target/vulkan-upload-baseline/trace.ron` and
`target/vulkan-upload-immediates/trace.ron`, summarized by
`scripts/analyze-vulkan-trace.py --from-frame 96`. The analyzer targets wgpu 29's
trace format. Tracing adds overhead and its timings are not performance evidence.

Release `preview_branding --vulkan --profile`, same 900 x 640 scene, 92 samples
after warm-up on the RTX 5070 Laptop GPU:

| Measurement | CPU-generated trail on Vulkan | Shader trail |
| --- | --- | --- |
| Median CPU scene recording | 217–225 µs | 5.4–8.5 µs |
| Median rendering plus screenshot readback | 1.59–1.65 ms | 1.62–1.80 ms |

Readback results include CPU synchronization and image transfer; they do not
establish a GPU speedup. Direct GPU timestamps are measured separately by
`profile_vulkan_resources`, with no image readback (only 16 timestamp bytes),
one/two independent scenes, 120 frames per scale, first 12 omitted. It serializes
completion to collect timings, so these are GPU command costs, not production
end-to-end frame latency or throughput.

This profile exposed a 16 MiB image atlas per window. The vendor default page is
now 1024 square: approximately 4 MiB initially and 8 MiB after the measured full
float/blink cycle, with existing growth/fragmentation retained. The two-window
scene saves 16 MiB of image textures. Device resource counts remain stable after
warm-up and textures are released when renderers close. Driver allocation pools
can retain reserved memory until the engine/device is dropped.

RTX GPU medians before/after the atlas change (shader enabled in both):

| Scale | One window, before | One window, after | Two windows, after (per frame/window) |
| --- | --- | --- | --- |
| 100% | 46.02 µs | 46.18 µs | 45.70 µs |
| 150% | 81.47 µs | 81.54 µs | 81.54 µs |
| 200% | 132.32 µs | 132.29 µs | 132.26 µs |

An earlier profiling run selected the AMD 610M integrated GPU; its timings are
kept separately and must not be compared with the RTX runs. The profiler now
explicitly requests high performance and prints adapter identity. Object counts
use wgpu registry reports: the upstream HAL texture counter returned negative
values on the integrated GPU and is not reliable evidence. Byte counts are
cross-checked against labeled allocator records.

The same profiler's `--editor` workload scrolls the actual document editor over
a Rust source fixture (three rows per frame), with separate CPU draw recording,
CPU command preparation, and GPU timestamps. One-window medians at 100/150/200%
were 150/154/153 µs recording, 151/181/132 µs preparation, and 20/34/51 µs GPU
work. Two-window per-frame GPU medians were also 20/33/51 µs. Resource object
counts stayed stable; glyph texture storage expanded once at 150% as new glyphs
were encountered. These measurements point to CPU text work, not GPU execution,
as the larger remaining scrolling cost in this fixture.

### Allocate quad storage and mesh pipelines on demand

The trace showed a 256 KiB solid buffer and 512 KiB gradient buffer per quad
layer, regardless of the scene's actual instance count. Quad layers now create
each type's buffer on its first nonempty batch, sized to that batch with the
existing power-of-two growth. Once allocated, buffers retain their high-water
capacity for reuse; an empty or smaller frame does not churn allocations.

RTX measurements at 100% scale, with the same shader/atlas settings before and
after this allocation change (`target/vulkan-quad-demand*-{before,after}.log`):

| Scene | Buffer bytes before | Buffer bytes after | Buffer objects before → after |
| --- | --- | --- | --- |
| About, one window | 4,319,888 | 3,534,464 | 19 → 18 |
| About, two windows | 8,115,168 | 6,544,320 | 36 → 34 |
| Editor, one window | 3,099,600 | 758,816 | 21 → 18 |
| Editor, two windows | 5,674,592 | 993,024 | 40 → 34 |

These device counter totals include the profiler's buffers and staging memory;
the before/after differences isolate the allocation change. About saves roughly
767 KiB per window; this editor fixture saves 2.23 MiB per window across three
quad layers. GPU medians remain effectively unchanged: About 46/82/132 µs and
editor 36/62/95 µs at scales 1/1.5/2. This fixture's absolute editor timings differ
from the earlier run, so only matched before/after results are compared.

The engine also defers its two mesh pipelines, optional MSAA pipeline, and
per-renderer mesh state until a visible mesh is prepared. An `Arc<OnceLock<_>>`
shares initialization across all engine clones, including clones created before
the first mesh. Meshes present during handoff initialize during the existing
offscreen warm-up; a later first mesh pays initialization at its first prepare.
The engine retains compiled pipelines until its last owner is released.
The final profiles confirm two fewer live pipelines: engine creation 5 → 3,
About 7 → 5, and editor 6 → 4, with unchanged buffer totals from the table above.
All pipelines are released after the renderers and engine are dropped.

`tests/vulkan_resource_reuse.rs` checks 4,096 instances of each quad type,
growth/shrink/empty frames, changing fills, clipped layers, independent windows,
and teardown. A second real-Vulkan regression checks deferred/shared solid and
gradient mesh pipelines, cached meshes, MSAA on/off, resize, and teardown.
It verifies that mesh pipelines prepared during offscreen warm-up are reused by
the next draw. Both regressions pass on Windows hardware Vulkan, SwiftShader,
and WSLg/Lavapipe. The AMD 610M also completes both final profiling workloads
with one/two windows at scales 1/1.5/2 (`target/vulkan-demand-amd*.log`).

### Lazy image resources and unused pipelines

Renderer construction now leaves the image cache and its worker uninitialized.
The first image operation creates the cache; the shared atlas texture and binding
are deferred further until pixels are uploaded into that atlas. RGBA measurement
requires no GPU texture. Images uploaded by the worker keep their independent
bindings without allocating the unused shared atlas. First shared-atlas upload
allocates all layers required by that image directly; later growth still copies
existing live layers and retains the allocation for reuse.

Image and gradient-quad pipelines are also initialized on demand and shared
across engine clones. Image use initializes its pipeline, while gradients compile
when a visible gradient batch is prepared. Engine creation now retains one
pipeline, down from three after the previous mesh-only deferral.

The same About and Rust-editor profiles (`target/vulkan-lazy-image*-{before,after}.log`)
confirm a 4 MiB reduction per newly created renderer, before it draws images.
The Rust fixture draws fold icons and therefore still needs an atlas once drawing
begins: its steady-state texture memory is unchanged. Both workloads retain one
fewer pipeline (About 5 → 4; Rust editor 4 → 3). The `--plain-text` profiler option
uses a `.txt` document without syntax/fold decorations to exercise image-free
editing separately.

That plain-text workload retains no image atlas throughout one/two-window
scrolling at all three scales. At 100% scale, both window counts report 2,662,400
texture bytes: the shared glyph atlas plus the profiler's render target. Two
pipelines remain (solid quads and text), with no image or gradient pipeline.
Its GPU medians are 41/70/104 µs at 100/150/200%; these describe this workload,
not a before/after speedup (`target/vulkan-lazy-plain-after.log`).

The image regression verifies empty renderer allocation, RGBA measurement,
shared pipeline initialization across windows, duplicate asynchronous allocation
callbacks, persistent allocation leases, synchronous atlas growth preserving old
pixels, a fragmented first upload spanning multiple pages, ordinary asynchronous
large-image upload, and resource teardown. Existing
quad tests additionally verify deferred gradient initialization and sharing.

### Compact atlas for small raster images

Raster images up to 128 pixels on each side now use a separate 256-square atlas.
Larger raster artwork and SVGs retain the 1024-square atlas. Both allocate only
when needed. A cache entry records `Main`, `Icons`, or `Dedicated` ownership;
lookup and eviction use that ownership instead of inferring a binding from the
presence of another atlas. Worker uploads retain their independent bindings.

Matched RTX profiles (`target/vulkan-icon-atlas*-{before,after}.log`) show:

| Workload at 100% scale | Texture bytes before | Texture bytes after |
| --- | --- | --- |
| Rust editor, one window | 6,856,704 | 2,924,544 |
| Rust editor, two windows | 11,247,616 | 3,383,296 |
| About, one window | 11,051,008 | 11,051,008 |
| About, two windows | 19,636,224 | 19,636,224 |

The fold-icon atlas shrinks from 4 MiB to 256 KiB, saving 3.75 MiB per editor
window. Totals include the shared glyph atlas and profiling render target. GPU
medians remain close: About 46/82/132 µs and Rust editor 36/61/95 µs at scales
1/1.5/2. CPU measurements varied between runs; this is a memory improvement,
not evidence of a CPU speedup. A scene using both image sizes may allocate both
atlases, adding a small page and binding changes compared with a single pool.

The resource regression renders three generations of 20 changing 96-square
icons alongside large artwork and a dedicated worker image. It exercises both
synchronous loading and ordinary draw uploads, atlas growth, eviction and reuse,
while checking that retained icon/artwork allocations keep the correct pixels.
The AMD 610M completes the same final Rust-editor workload at all three scales
with one/two windows. Plain-text profiling still allocates no image atlas.
Those follow-ups ran alongside validation and are correctness/resource evidence,
not isolated timing comparisons (`target/vulkan-icon-atlas-{amd-editor,plain}.log`).

### Device limits and allocation failure

A regression using a Vulkan device requested with two texture-array layers and
a 2048-pixel texture dimension reproduced an unchecked atlas growth failure:
`Dimension Z value 4 exceeds the limit of 2` (`target/vulkan-atlas-limits-before.log`).
Atlas allocation now observes the device's requested limits. If one fragment
cannot fit, all earlier fragments from that attempt are released and newly added
logical layers are removed before any upload commands are recorded.

A full icon atlas spills into the main atlas. If shared storage cannot fit an
image, a separately owned texture is used when the image fits the device by
itself. Ownership remains explicit for drawing, trimming and teardown. If that
also exceeds the limits, synchronous loading returns an allocation error without
submitting an empty command buffer. The upload worker reports the error and wakes
the application so asynchronous allocation callbacks also complete.

The real-Vulkan regression checks fragmented rollback, reuse of both available
main-atlas layers, icon-to-main spill, independent textures after both pools fill,
ordinary drawing and explicit loading, and synchronous/asynchronous failures for
a 4096-square image. Existing images retain their pixels after these failures.
This tests API/device limits, not physical-VRAM exhaustion or a driver device loss.
The final About/editor resource profiles retain the previous object and byte
totals (`target/vulkan-atlas-limits-{about,editor}.log`).

Ten release startup runs on Windows at 150% scaling used independent settings
and cache directories. Median/p95 app-reported first view was 35.6/39.9 ms;
first rendered screenshot was 141.4/153.3 ms. Process-launch-to-probe was
158.3/326.3 ms (the first process launch was substantially slower). Traces record
tiny-skia presentation and no Vulkan handoff during this empty-document startup.
The screenshot timing is not itself a presentation-latency measurement. These
are warm OS-cache observations, not a cold-boot distribution or a before/after
startup improvement claim.

### Sustained live cadence and redraw costs

The strict handoff probe now supports `--sustain-seconds=12`, `--editor`, and
`--plain-text`. It switches from its lifecycle subscriptions to the workload only
after verified first Vulkan presentation. The sustained view has no changing
probe labels or frame subscription that would force extra redraws. About keeps
its own 24 Hz scheduler; editor scrolling advances three rows per 24 Hz timer
tick. About pauses on focus loss, so its sustained capture uses one window.

`scripts/profile-vulkan-live.py` runs the release binary in an isolated trace
directory, discards two seconds after handoff, and summarizes the next ten.
It requires strict Vulkan warm-up/first-presentation evidence and a successful
Vulkan presentation within every accepted redraw. It rejects software fallback,
failed presentation, missing windows, incomplete intervals, and insufficient samples. Six analyzer
regressions cover buffered trace order and invalid/missing evidence; both CI
scripts run them. The CSV writers flush independently, so analysis uses
timestamps rather than physical file order.

Sequential RTX 5070 Laptop GPU runs at 150% scaling (1350 x 960 physical pixels),
240 measured redraws per window:

| Workload | Cadence | CPU redraw median / p95 | Presentation call median / p95 |
| --- | --- | --- | --- |
| About | 24.00 fps | 0.530 / 0.689 ms | 0.464 / 0.609 ms |
| Rust editor | 24.00 fps | 0.877 / 1.136 ms | 0.576 / 0.746 ms |
| Rust editor, two windows | 23.99 fps each | 0.631–0.858 / 0.873–1.178 ms | 0.409–0.591 / 0.616–0.807 ms |
| Plain-text editor | 24.00 fps | 0.754 / 1.013 ms | 0.578 / 0.768 ms |

No measured interval exceeded 62.5 ms. These are instrumented redraw wall times
(interaction/drawing/presentation, including tracing and presentation waits).
Separate application updates and UI rebuilding are outside that redraw interval.
They do not measure GPU execution, display scanout latency, maximum throughput,
or an optimization's before/after speedup. The probe's prepare/commit delays are
outside the measurement interval. Evidence directories under `target/vulkan-live/`:
`about-52d1x_rg`, `editor-j5vulxq6`, `editor-kinou7kk`, `plain-text-mphh1j0f`.

Windows SwiftShader also retains 24.00 fps over 240 measured About redraws at
150% scaling, with median/p95 CPU redraw costs of 8.044/9.300 ms and no interval
over 62.5 ms (`about-hu4cwwpt`). This is a software-Vulkan comparison, not a
physical-GPU result.

WSLg X11 + Lavapipe release captures at 100% scaling (900 x 640 physical pixels)
also retain 24.00 fps over 240 measured frames. About median/p95 CPU redraw cost
is 3.429/4.042 ms; Rust editor is 3.446/4.197 ms, with no interval over 62.5 ms.
Evidence: `about-palyoupm` and `editor-7tcp3mt_`. These runs unset
`WAYLAND_DISPLAY` and `WAYLAND_SOCKET` only in the child shell to select X11.
A separate Wayland editor capture also passes (`editor-751ngr55`, 24.00 fps,
3.392/4.079 ms). Linux About over Wayland has an unresolved startup connection
failure described below, so the X11 results do not establish Wayland reliability.

The first sustained editor attempt exposed a shared tracing bug: application
trace initialization unlinked the file after Iced had opened its writer, losing
the windowing/handoff events. Application tracing now appends; capture runners
reset or choose a fresh path before startup. A regression keeps an existing
renderer writer alive across application logger initialization and checks both
writers' events remain readable. The rejected capture remains in
`target/vulkan-live/editor-e_k1fvmx`. An earlier probe-only attempt timed out
because its initial frame subscription was removed too early; that subscription
now remains until handoff completes (`about-poagvhyc`).

### Apply synchronous window resize results

The strengthened resize scenario reproduced a stale viewport on Wayland:
requesting 700 x 440 left Iced at 640 x 380, including its Vulkan warm-up target.
Winit explicitly documents that `request_inner_size` can return the applied size
without a later resize event. Its Wayland implementation follows that path;
Iced previously discarded the return value.

The runtime now updates window state from that returned physical size, queues
the logical resize event, and requests redraw. Existing redraw processing
relayouts the UI and reconfigures the surface. It uses the returned size rather
than the requested size, respecting window-system clamping/rejection. Async
resize requests continue through their normal window events.

Before/after evidence is in `target/vulkan-resize-{before,after}/`: the stronger
probe rejects the old behavior and observes 700 x 440 after the fix. The matrix
runner additionally requires successful software presentation at the requested
logical size before warm-up, with the same physical dimensions used by Vulkan.
A Rust probe regression rejects initial/wrong-window resize events; a Python
evidence regression rejects stale dimensions, failed presentation, and software
frames outside the prepare interval. Both CI scripts run the Python check.

The separate release Wayland connection error has now been traced to WSLg's
compositor, which exits with signal 11 after the first software frame. The app's
socket then returns EOF; Vulkan warm-up has not started. Server logs captured
before WSLg restarts are retained in `target/vulkan-wayland-server-evidence/`,
including the WSLGd `pid 13 terminated with signal 11` record. System compositor
and driver settings were not changed. This identifies the crashing process;
the specific Weston defect has not been diagnosed or fixed here.

An isolated, locally extracted Weston 14.0.2 running on WSLg X11 with a Pixman
host renderer passes all nine release handoff scenarios and both sustained
About/editor captures (`target/weston14-loh1q22z/`). About retains 24.00 fps over
240 measured redraws, with 3.587/4.550 ms median/p95 CPU redraw cost. This is
Lavapipe software Vulkan, not a physical-GPU measurement.

`scripts/check-wayland-vulkan.py` now reproduces this validation with a private
socket/runtime directory and automatic compositor teardown. Its default
headless Weston/Pixman path also passes all nine handoff scenarios and the
About/editor captures (`target/vulkan-wayland/run-88hjeg7a/`). Linux CI and nightly
validation install Weston, build the release probe, run this path in addition to
X11 tests, and retain the compositor logs alongside handoff/live traces.

## Platform validation and distribution

- Windows RTX: all nine strict handoff scenarios passed with real About artwork.
- Windows SwiftShader: shader/resource tests and all nine strict scenarios pass.
- Debian WSLg + Lavapipe: shader/resource tests and all nine strict scenarios pass
  in the debug Wayland and release X11 runs. The strengthened resize check passes
  on isolated Weston 14 Wayland in release mode. The system WSLg Weston still
  crashes in some release scenarios; its signal 11 is captured separately.
- Windows standard CI checks pass under the configured SwiftShader runtime
  (724 Rust tests and five asset tests), including
  software-only and example checks, after the synchronous-resize fix. The six
  live-trace analyzer tests and resize-evidence Python test also pass.
  Targeted physical-GPU rendering
  tests and the nine NVIDIA handoff scenarios also pass.
- Linux `cargo test --locked --all-targets` passes (738 Rust tests) after the
  synchronous-resize fix. An earlier push-constant validation run hit one
  startup timeout; the isolated retry and both subsequent complete reruns passed.
- The application-locked `iced_wgpu` regression test passes. The separate vendor
  workspace lock resolves an older glam version and fails before reaching tests;
  use the application lock for this validation.
- The exported patch reverse-checks and reproduces all 42 patched files in a
  fresh checkout of the existing vendor pin.
- Native macOS and physical-GPU Linux results remain missing.

After enabling push constants, all 27 strict handoff cases passed again:
RTX `run-qub9w6tx`, SwiftShader `run-_umolx5p`,
and WSLg/Lavapipe `run-ym_esjfn`, under `target/vulkan-handoff/`. SwiftShader also
passes all four targeted Vulkan rendering/resource tests. Both the RTX 5070 and
AMD 610M complete the 240-frame offscreen profile with push constants and with
the explicit `--uniforms` fallback. These are correctness/resource observations;
the initial timing comparison overlapped other validation work and does not
establish an isolated performance improvement.

After the quad-allocation and lazy-mesh changes, all 27 strict handoff cases
passed again. Final logs are `target/vulkan-demand-{rtx,swiftshader,linux}-handoff.log`;
each log identifies its separate evidence directory. Windows and Linux full
suite logs are `target/vulkan-demand-windows-ci.log` and
`target/vulkan-demand-linux-tests.log`. Software-only example checks and the
application-locked `iced_wgpu` test also pass.

The lazy-image/gradient follow-up passes all 27 strict cases again: RTX
`run-l_svpt3o`, SwiftShader `run-tv3ytfih`, and WSLg/Lavapipe `run-rqqe254l`.
Full suites are recorded in `target/vulkan-lazy-{windows-ci,linux-tests}.log`;
final resource regressions are `target/vulkan-lazy-final-*-test.log`. The AMD
610M completes the final About and plain-text profiles at all three scales with
one/two windows (`target/vulkan-lazy-amd-{about,plain}.log`); those runs overlapped
validation and are correctness observations, not isolated timing comparisons.

Compact-icon-atlas validation is recorded in
`target/vulkan-icon-windows-swiftshader-ci.log`,
`target/vulkan-icon-linux-tests.log`, and
`target/vulkan-icon-{rtx,swiftshader,linux}-handoff.log`. The vendor regression
also passes. Two earlier Windows runs failed: the startup check measured a
520 ms first view against its 200 ms budget during concurrent validation (the
isolated retry was 31.7 ms), and a later hardware run could not initialize a
Vulkan adapter in one test (`active_backends=0`). The four targeted hardware
Vulkan tests then passed, followed by the NVIDIA handoff matrix. The cause of
the one adapter-initialization failure is unconfirmed; those failed logs remain
at `target/vulkan-icon-windows-ci{,-retry}.log`.

Device-limit validation passes all 27 strict handoff cases: RTX `run-oavhe1ar`,
SwiftShader `run-q21zu_w0`, and WSLg/Lavapipe `run-bxhyly3c`. Logs are
`target/vulkan-limits-{rtx,swiftshader,linux}-handoff.log`; full-suite logs are
`target/vulkan-limits-windows-ci.log` and `target/vulkan-limits-linux-tests.log`.
The final strengthened rollback test, which allocates existing content before
the failing fragmented image, also passes on NVIDIA and SwiftShader separately
and in the Linux full suite. The vendor regression and software-only example
checks pass. The exported vendor patch reproduces all 42 files again.

Sustained-probe/shared-trace validation logs are
`target/vulkan-live-windows-ci.log`, `target/vulkan-live-linux-tests.log`, and
`target/vulkan-live-{rtx,swiftshader}-handoff.log`. The four existing probe unit
tests and software-only example checks pass; the final six Python analyzer tests
pass on Windows and Linux. Handoff directories: NVIDIA `run-zt0rpsl3`,
SwiftShader `run-p7ymqep8`, WSLg Wayland debug `run-dpj7t4l7`, and WSLg X11 release
`run-3e2ipys0`.

Two release Wayland matrices failed during resize with `Broken pipe (os error 32)`
and `WindowCreationFailed(ExitFailure(1))` before recording a result. Their other
eight cases passed (`run-myotnjes`, `run-069hgqt0`). A release About capture failed
the same way before GPU handoff (`about-4lr5f4b4`). A targeted resize run with
`WAYLAND_DEBUG=1` passed (`target/vulkan-live-resize-wayland-debug/`), but its added
logging changes timing and does not prove a fix. No renderer or window-system
workaround was applied; the cause remains unconfirmed.

Inspecting those traces also exposed a limitation in the former resize probe:
it accepts any resize event, including the initial 640 x 380 size, rather than
requiring the requested 700 x 440 size. X11 records the requested size; the passing
Wayland debug run does not. The ignored `request_inner_size` result is now fixed
and the probe/trace verifier requires the requested size (see the before/after
evidence above). The older matrix passes did not prove applied resizing on
Wayland. The compositor signal 11 is independent of that fix and remains an
environment limitation.

CI configures Windows SwiftShader, Linux Lavapipe, and macOS MoltenVK explicitly.
The final resize validation passes 36 handoff checks across NVIDIA
(`run-_0uh73_n`), SwiftShader (`run-ztm7bupu`), WSLg X11/Lavapipe (`run-crtusx6o`),
and headless Weston 14/Lavapipe (`target/vulkan-wayland/run-88hjeg7a/handoff/run-22e9a4ar`).
All resize cases require the requested event, software presentation, and matching
physical warm-up size. Full-suite logs are `target/vulkan-resize-windows-ci.log`
and `target/vulkan-resize-linux-tests.log`; the five probe unit tests also pass
on Windows. The exported patch reproduces all 42 files in
`target/iced-resize-verification-20260922`. Workflow YAML parses successfully;
hosted CI execution is still pending.

The cross-platform matrix runner requires actual Vulkan warm-up and per-window
presentation evidence; it rejects indeterminate results. Diagnostic JSON, CSV,
and logs are written under `target/` for local inspection.

macOS packaging now includes a lazy launcher, Vulkan loader, MoltenVK, relative
ICD manifest, library relocation/signature checks, runtime checksums, and notices
from the exact installed formula's sources and pinned static dependencies.
Source/checksum/notice collection was exercised for the current formulae.
Shell syntax and workflow YAML pass local validation; actual macOS loading,
signing, relocation, Gatekeeper behavior, and presentation await a native run.
See [PACKAGING.md](PACKAGING.md) for commands and distribution layout.

Useful reproduction commands:

```text
cargo run --release --example preview_branding -- --vulkan --profile
cargo run --release --features wgpu/counters --example profile_vulkan_resources
cargo run --release --features wgpu/counters --example profile_vulkan_resources -- --editor
cargo run --release --features wgpu/counters --example profile_vulkan_resources -- --plain-text
cargo run --release --features wgpu/counters --example profile_vulkan_resources -- --low-power
cargo run --release --features gpu-profiling --example profile_vulkan_resources -- --single-scene --frames=240 --trace-dir=target/new-vulkan-trace
python scripts/analyze-vulkan-trace.py target/new-vulkan-trace/trace.ron --from-frame 96
cargo build --example backend_switch_probe
python scripts/check-vulkan-handoff.py --binary target/debug/examples/backend_switch_probe.exe
cargo build --release --example backend_switch_probe
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe
python scripts/profile-vulkan-live.py --binary target/release/examples/backend_switch_probe.exe --workload editor --windows 2
python scripts/check-wayland-vulkan.py --binary target/release/examples/backend_switch_probe
cargo test --locked -p iced_wgpu --lib
python scripts/profile-startup.py --binary target/release/fragile-notepad.exe
```

Use the Unix executable path without `.exe` on Linux/macOS, after configuring
the relevant loader/ICD. Raw local logs are under `target/vulkan-*.log` and each
matrix execution has its own directory under `target/vulkan-handoff/`.

## Quad and image transform push constants

Quad and image transforms now use optional 80-byte and 64-byte Vulkan push
constants, respectively. Devices requested without the feature or with smaller
limits retain the existing uniform path for each pipeline. Image layers share
the engine's nearest/linear sampler bindings on the immediate path. The
software-first handoff sequence is unchanged.

`tests/vulkan_transform_parameters.rs` compares every pixel against the uniform
path with requested limits 0, 16, 64, and 80 bytes, two independent renderers,
three clipped layers each, solid/gradient quads, both image filters, opacity,
rotation, changing transforms, scales 1/1.5/2, empty frames, and teardown.
NVIDIA RTX 5070 Laptop, AMD 610M, Windows SwiftShader, and Linux Lavapipe pass.
The fixture retains 30 buffers/23 bindings at limits 0 and 16, 24/13 at 64,
and 18/7 at 80. Logs: `target/vulkan-transform-{rtx,amd,swiftshader,linux}-test.log`.

The actual Rust editor scroll trace at frames 96 through 239 removes all 409
quad-transform copies (32,720 bytes) and 144 image-transform copies (9,216 bytes).
Solid instances (842,500 bytes), image instances (71,064 bytes), glyph vertices
(3,277,372 bytes), and glyph-atlas writes (924 bytes) are identical before and
after; image-atlas uploads remain zero. The 20,054,016 bytes of mapped staging
ranges are also unchanged and are not GPU transfer volume. The optimization
removes 553 small copies and their resources, not the moving text's uploads.
Evidence: `target/vulkan-editor-upload-{baseline,immediates}.json` and matching
trace directories/logs.

Actual About/editor offscreen workloads pass with one/two windows, scales
1/1.5/2, and both parameter paths (240 frames per scenario), exercising mixed
text, images, quads, and the About trail. Logs:
`target/vulkan-transform-{about,editor}-{immediates,uniforms}.log`.
These runs use the profiling build without API trace capture. GPU times remain
similar (About roughly 25/44/71 microseconds and editor 20/33/51 at the three
scales); this is not evidence of a general frame-latency improvement.

## Still required before completing the objective

Native macOS package/loader/driver verification and physical-GPU Linux testing
remain required. Broader editor workloads, live frame latency, cold startup
distributions, GPU upload counts for workloads beyond the About scene, and
integrated GPU power/throughput measurements also remain. The current evidence supports
the specific CPU and memory improvements above, not completion of the full
cross-platform optimization objective.

Sustained live About/editor cadence and CPU redraw/presentation-call costs are
now measured separately from the existing offscreen GPU timestamps. Longer
captures, broader editor workloads, resizing, and maximum-throughput/power
measurements remain; the live timestamps cannot establish display scanout latency.

Requested texture-array exhaustion is now covered by a constrained real-Vulkan
device regression. Actual driver allocation failure/device loss and physical
VRAM pressure are not covered by that test.

Requested resizing now has direct software-presentation and warm-up-size
evidence. The old WSLg compositor crash is isolated from the passing Weston 14
Wayland and X11 paths; native Linux physical-GPU and macOS validation remain.
