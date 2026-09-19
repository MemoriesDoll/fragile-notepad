# Seamless Hybrid Rendering Plan

## Goal

Requirement: Fragile Notepad keeps its current software startup path and uses lazy hybrid rendering as the normal runtime policy. GPU rendering is initialized after startup, primarily for animation-heavy interactions, while CPU-only rendering remains available as an explicit fallback and override. The target behavior is:

- launch with the current tiny-skia/software backend;
- switch to wgpu on demand without closing or recreating app windows;
- enforce, by instrumentation and tests, that once handoff begins, no additional software frame is presented after the last software frame;
- request and present the first GPU frame immediately after the backend swap completes;
- keep the app responsive and visually stable across the switch;
- degrade cleanly when GPU initialization fails.

This is not "GPU acceleration inside tiny-skia." The model under evaluation is software-first rendering with a runtime switch from `Backend::Software` to `Backend::Hardware(Api::Best)` when both `iced/tiny-skia` and `iced/wgpu` are compiled in.

## Feasibility Summary

This document is feasible only if the work is split into two tracks:

1. **Basic lazy GPU switch.** This is feasible with the current vendored Iced API. It can use `backend::configure` to switch an existing app from software to hardware in-process, then request/redraw the next frame. This track has been promoted from opt-in experiment to the default compiled capability, with `--no-default-features` retained for CPU-only builds.
2. **Strict seamless handoff.** This is not feasible as an app-only change. The vendored Iced runtime now has a prepare/warm/commit path with real wgpu offscreen warm-up, strict trace evidence, renderer identity instrumentation, and rollback reporting. It still needs platform validation and remaining lifecycle fixes before it can be described as release-ready.

Do not ship or describe the strict seamless behavior as implemented by simply adding `iced/wgpu`, using `backend::configure`, or increasing GPU queue depth. Those pieces are useful for the basic switch, but strict success now requires the prepare/warm/commit path plus trace evidence that real offscreen warm-up completed before commit and that frame order was preserved.

## Definition Of Seamless

For this project, "seamless" has a strict frame-order meaning:

```text
software frame N is presented
backend handoff runs
GPU frame N+1 is requested immediately
GPU frame N+1 is presented
```

The acceptance target is no observed blank frame, no app-window recreation, no hidden-window replacement, and no extra software animation frame after the handoff starts. These are requirements to verify; they are not established by the current probe.

There is one important boundary: the OS compositor, GPU driver, and wgpu device/surface creation time prevent a literal zero-millisecond guarantee. The project can only target and verify frame sequence and immediate redraw scheduling from Fragile Notepad's side: the last frame submitted by Fragile Notepad before handoff is software, and the next frame submitted by Fragile Notepad after handoff is GPU.

This means lazy initialization must happen before the user-visible animation begins. The app can show the last software frame while GPU setup runs, but it must not start an animation on software and then switch halfway through that animation.

For cold GPU initialization, "seamless" also means software remains the active producer until the GPU path has warmed up. If GPU initialization or first-frame compilation stalls, the required behavior is that the user keeps seeing software-presented frames or a stable final software frame. The app must not commit to the GPU renderer until an instrumented check shows the GPU path can produce the next frame.

## Evidence Ledger

Use this section to prevent overstatement.

Verified locally:

- product code compiles with default features and with `--no-default-features`;
- the backend switch probe switches from software to hardware and reaches a later frame;
- the backend switch probe exits with status 0 after printing the expected switch markers;
- vendored Iced has a public `backend::configure` task;
- vendored Iced's winit backend replaces compositor, renderers, and surfaces for existing windows;
- vendored Iced has `backend::prepare_warm_and_commit` / `Action::PrepareWarmAndCommit`;
- `Compositor::warm_up_offscreen` is implemented as real wgpu offscreen work and returns `OffscreenWarmUpEvidence`;
- tiny-skia reports offscreen warm-up as unsupported, so strict warm-up evidence must come from wgpu;
- the Windows single-window strict probe reports `result=ok` with strict outcome success, Wgpu/Vulkan presented evidence, and non-null `warm_complete_us`;
- the Windows multi-window strict probe reports `result=ok` with strict outcome success, Wgpu/Vulkan presented evidence for both live windows, and non-null warm evidence;
- the Windows close-during-preparing probe reports `result=ok` for the intentional `Cancelled + Preparing + NotNeeded` cancellation path;
- WSL/Linux compile checks and targeted startup/lifecycle tests pass outside the sandbox, with only vendored `encoding_rs` lifetime syntax warnings during Cargo commands.

Not verified yet:

- complete IME dispatch evidence across renderer replacement; current tests do not include a private `handle_event` end-to-end IME dispatch hook;
- WSL/Linux strict hardware proof; GUI prerequisites are present, but adapter creation is blocked by `GraphicsAdapterNotFound` / no suitable adapter and should be treated as an environment blocker rather than source pass evidence;
- native macOS behavior; the macOS path is prototype-only and has not been locally validated;
- there is no visible flash on the supported platform matrix;
- full release readiness across resize, secondary windows, IME, close/exit, GPU failure, and platform-specific driver behavior.

Every item in the "not verified yet" list is a release-blocking validation item for the strict seamless-animation goal.

## Current Code Context

### Startup

`src/startup.rs` currently forces software rendering:

```rust
pub fn iced_settings() -> Settings {
    Settings {
        backend: Backend::Software,
        antialiasing: false,
        vsync: false,
        ..Settings::default()
    }
}
```

This is the current desired startup default. It avoids making first paint depend on adapter discovery, device creation, and driver behavior.

### Iced Features

`Cargo.toml` currently enables:

```toml
iced = { path = "vendor/iced", default-features = false, features = ["tiny-skia", "tokio", "image-without-codecs", "highlighter", "advanced", "x11", "wayland"] }
```

`hybrid-rendering` is enabled by default and pulls in `iced/wgpu`. A product compile check with `--no-default-features` still succeeds, which means the CPU-only tiny-skia path remains available when the default hybrid feature is disabled.

The examples compile under the default hybrid feature set. The backend switch probe requires `hybrid-rendering`; when run with `--no-default-features`, it prints a skip marker instead of failing to compile.

### Message Routing

`src/message.rs` contains the app's message enum. Backend switching should be represented explicitly here so the operation is observable and can update app state:

```rust
BackendBoostRequested,
BackendBoostConfigured(Result<(), iced::backend::Error>),
```

The exact names can change, but the important part is that the runtime handoff task must be mapped back into app messages. That message boundary is what lets the app record preparation, warm-up, success/failure, redraw readiness, and duplicate switch attempts.

### App State

`src/app.rs` owns the main state and routes messages through `update_inner`. Add a small rendering state to `App`:

```rust
rendering: RenderingState,
```

Suggested shape:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderingState {
    Software,
    GpuPreparing,
    GpuWarming,
    CommitPending,
    Hardware,
    Failed,
}
```

This is intentionally simple. The app does not need to know which physical adapter was chosen for the first implementation. If telemetry or diagnostics are added later, the state can grow without changing the switching contract.

### Redraw And Animation Hooks

`src/editor/widget/interaction.rs` already uses `Shell::request_redraw()` and `Shell::request_redraw_at(...)` for caret blinking, focus changes, selection dragging, and input method updates. That pattern is the right way to drive UI animation frames in widgets.

For app-level animation triggers, use one of these approaches:

- widget-local redraw scheduling through `Shell`;
- an app subscription if animation state is global;
- a targeted task that updates animation state and then relies on the view/update cycle.

Avoid introducing GPU switching as an animation clock. The backend switch should happen once, before or at the beginning of animation-heavy behavior.

## Verified Runtime Behavior

A temporary runtime probe was used to verify the switch path:

1. start with `Settings { backend: Backend::Software, ... }`;
2. wait for a rendered frame through `window::frames()`;
3. call:

```rust
backend::configure(backend::Settings {
    backend: Backend::Hardware(Api::Best),
    antialiasing: false,
    vsync: true,
})
```

4. map the result into an app message;
5. wait for a post-switch frame;
6. exit.

The probe printed:

```text
BACKEND_SWITCH_PROBE_START
BACKEND_SWITCH_PROBE_CONFIGURED
BACKEND_SWITCH_PROBE_POST_SWITCH_FRAME
```

and exited with status 0.

This verifies only that a software-to-wgpu switch completed in-process in the local probe and that a later frame was produced without recreating the app. It does not verify renderer identity for that later frame.

## Iced Internals That Make This Possible

### Runtime Action

`vendor/iced/runtime/src/backend.rs` exposes:

```rust
pub fn configure(settings: backend::Settings) -> Task<Result<(), backend::Error>> {
    task::oneshot(|sender| crate::Action::Backend(Action::Configure(settings, sender)))
}
```

This gives the app a normal `Task`-based integration point.

### Winit Reconfiguration

`vendor/iced/winit/src/lib.rs` handles `backend::Action::Configure` by:

- creating a new compositor with the requested settings;
- invalidating graphics caches;
- replacing each window renderer and surface;
- storing the new compositor;
- sending `Ok(())` or the creation error back through the task channel.

The important implementation detail is that existing windows stay managed by the same app instance. The renderer and surface are swapped under the window manager.

For diagnostics, `backend::configure` remains useful as the older basic switch path. It is not strict proof because it combines preparation and commit and does not provide offscreen warm-up evidence. For the strict seamless requirement, use the prepare/warm/commit runtime action and require trace/result evidence from that path. The app message remains useful for state tracking, but the first GPU frame must be scheduled and proven by the runtime handoff rather than by an incidental later UI event.

## Feasibility Assessment

The basic in-process software-to-hardware switch is verified in the local probe. The strict CPU-active warm-up architecture has now moved from proposal to implementation in the vendored Iced runtime: the prepare/warm/commit action keeps software active during preparation, runs real wgpu offscreen warm-up through `Compositor::warm_up_offscreen`, records strict evidence, and reports structured success/failure. Treat the implementation as validation-limited rather than release-ready until the gates in this document pass.

Practical feasibility is split into three tiers:

1. Basic runtime switch: implemented with the vendored Iced API.
2. Immediate redraw after switch: implemented for the runtime handoff path.
3. Strict seamless handoff with CPU-active GPU warm-up: implemented in the vendored Iced fork, but still gated by strict probe evidence and platform validation.

Do not treat tier 3 as a small app-layer change. The public `backend::configure` API creates and commits the new compositor in one blocking operation, so it cannot by itself prove that software remains active while GPU preparation runs. Use `prepare_warm_and_commit` for strict validation.

### Level 1: Proven Runtime Switch

Status: verified enough for an internal prototype with the current vendored Iced architecture.

Evidence:

- the product app compiles with `iced/wgpu`;
- a runtime probe switched from `Backend::Software` to `Backend::Hardware(Api::Best)`;
- the probe reached a post-switch frame and exited 0;
- Iced already replaces the compositor, renderer, and surface without recreating app windows.

Limitations:

- the current switch can stall while wgpu creates the instance, adapter, device, and surface;
- the current switch does not prove CPU keeps presenting during GPU preparation;
- the current switch does not prove the first post-switch presented frame is GPU by runtime instrumentation.

This level is suitable for an internal prototype and diagnostic builds. It is not enough for the strict seamless-animation requirement.

### Level 2: CPU-Active GPU Warm-Up

Status: implemented in the vendored Iced runtime and validated by the single-window strict probe when trace evidence is enabled. It is not yet a release claim for the full platform and lifecycle matrix.

The key constraint is in `vendor/iced/winit/src/window.rs`: each `Window<P, C>` currently owns one active `surface: C::Surface` and one active `renderer: P::Renderer`. With both backends enabled, `C` is a fallback compositor and `P::Renderer` is a fallback renderer, but the fallback types are single-choice enums:

```rust
Compositor::Primary(wgpu) | Compositor::Secondary(tiny_skia)
Surface::Primary(wgpu) | Surface::Secondary(tiny_skia)
Renderer::Primary(wgpu) | Renderer::Secondary(tiny_skia)
```

They do not represent "active tiny-skia plus pending warmed wgpu" at the same time.

Therefore, CPU-active GPU warm-up uses transition storage outside the existing active `Window` fields. The implementation follows the shape of:

```rust
struct PendingGpuHandoff<C, R> {
    compositor: C,
    renderers: FxHashMap<window::Id, R>,
    warmed: bool,
    started_at: Instant,
}
```

where the active window manager keeps using tiny-skia until commit. At commit, the runtime replaces active renderers/surfaces in one present-boundary operation.

The pending compositor and renderer types are kept separate from `Window<P, C>` so the existing generic `Compositor<Renderer = P::Renderer>` contract and the single-choice fallback renderer enum remain intact.

### Surface Feasibility Constraint

Do not assume it is valid to hold two live presenting surfaces for the same OS window on every platform.

The implemented lower-risk path is:

- prepare the wgpu instance, adapter, device, queue, and renderer while tiny-skia remains active;
- avoid creating a long-lived visible wgpu surface for the same window until commit unless platform testing verifies it is valid;
- perform warm-up using real offscreen GPU work through `Compositor::warm_up_offscreen`;
- create/configure the actual visible wgpu surface at the commit boundary;
- immediately request redraw and present the first visible GPU frame.

If offscreen warm-up is not sufficient to eliminate first visible surface cost on a platform, the feature can still be acceptable for delayed animation startup only if tests show the last software frame stays on screen and the animation clock does not start until after the first GPU present. It should not be described as zero-stall.

### Feasibility Verdict

Current verdict:

- the basic runtime switch is verified;
- strict prepare/warm/commit handoff and offscreen wgpu warm-up are implemented in vendored Iced;
- current Windows single-window strict validation reports `result=ok`, strict outcome success, Wgpu/Vulkan presented evidence, and non-null `warm_complete_us`;
- current Windows multi-window strict validation reports `result=ok`, strict outcome success, Wgpu/Vulkan presented evidence for both live windows, and non-null warm evidence;
- current Windows close-during-preparing validation reports `result=ok` for intentional cancellation during `Preparing`;
- the strict handoff is not release-ready if implemented as only `backend::configure` plus triple buffering;
- the strict handoff is not release-ready without trace evidence proving renderer identity for the final software frame and first GPU frame;
- cross-platform strict proof remains limited by WSL/Linux GPU adapter creation failing with `GraphicsAdapterNotFound` / no suitable adapter, and by macOS remaining prototype-only;
- native macOS remains prototype-only and not locally validated;
- the strict handoff is not release-ready without a kill switch and automatic rollback to software on GPU errors.

## Proposed Architecture

### Default To Software

Keep `src/startup.rs` as software-first:

```rust
backend: Backend::Software,
antialiasing: false,
vsync: false,
```

This keeps first-view readiness stable and preserves the existing startup probe budget.

### Compile Both Backends

Enable `wgpu` in the Iced feature list:

```toml
features = [
    "tiny-skia",
    "wgpu",
    "tokio",
    "image-without-codecs",
    "highlighter",
    "advanced",
    "x11",
    "wayland",
]
```

Because startup still requests `Backend::Software`, enabling the feature is expected not to force GPU initialization at launch. Confirm this with startup tracing before treating it as a release property.

### Add A Rendering Module

Add a small module, for example `src/app/rendering.rs`, to keep backend state and tasks out of the large `App::update_inner` match.

Responsibilities:

- expose `request_gpu_boost(&mut self) -> Task<Message>`;
- guard against duplicate requests;
- map the backend switch task into rendering messages;
- update state on success/failure;
- optionally publish a visible status message on failure only.

Sketch:

```rust
use iced::backend::{self, Api};
use iced::{Backend, Task};

use crate::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RenderingState {
    Software,
    GpuPreparing,
    GpuWarming,
    CommitPending,
    Hardware,
    Failed,
}

impl App {
    pub(super) fn request_gpu_boost(&mut self) -> Task<Message> {
        if self.rendering != RenderingState::Software {
            return Task::none();
        }

        self.rendering = RenderingState::GpuPreparing;

        // Existing Iced API. This supports the basic lazy switch only.
        // It does not prove CPU-active warm-up or strict seamless handoff.
        backend::configure(backend::Settings {
            backend: Backend::Hardware(Api::Best),
            antialiasing: false,
            vsync: true,
        })
        .map(Message::BackendBoostConfigured)
    }

    pub(super) fn complete_gpu_boost(
        &mut self,
        result: Result<(), backend::Error>,
    ) -> Task<Message> {
        match result {
            Ok(()) => {
                self.rendering = RenderingState::Hardware;
                // Trigger a state change/redraw path after renderer replacement.
                Task::none()
            }
            Err(error) => {
                self.rendering = RenderingState::Failed;
                self.file_status = Some(format!("GPU acceleration unavailable: {error}"));
                Task::none()
            }
        }
    }
}
```

The code above is illustrative. The final implementation should use the repo's module visibility and avoid exposing details outside `app`.

The production strict path should use the prepare/warm/commit runtime action, not the basic `backend::configure` sketch above. `GpuWarming` and `CommitPending` count as evidence only when they come from the strict runtime action and the trace/result JSON proves warm-up completion and frame ordering.

### Add Messages

Add rendering messages to `src/message.rs`:

```rust
BackendBoostRequested,
BackendBoostConfigured(Result<(), iced::backend::Error>),
```

Then route them in `src/app.rs`:

```rust
Message::BackendBoostRequested => self.request_gpu_boost(),
Message::BackendBoostConfigured(result) => self.complete_gpu_boost(result),
```

If `iced::backend::Error` is not `Clone`, wrap the result in a local enum or convert failure to a string in the task mapping. Do not force `Message: Clone` to carry a non-cloneable error.

### Decide What Triggers The Boost

Start conservative. Do not switch to GPU at startup. Trigger only when there is a clear reason.

Initial candidates:

- opening a UI surface with continuous animation;
- first use of a command palette, panel transition, overlay animation, or future animated tab interaction;
- a user setting like "Enable hardware acceleration for animations";
- a manual diagnostic command during development.

Recommended first implementation:

1. add a hidden/manual command or debug-only trigger;
2. add one real animation feature that requests the boost before starting;
3. keep fallback behavior identical if the boost fails.

### Redraw After Successful Switch

There are two viable approaches.

Approach A: app-level redraw through normal state change.

- On `BackendBoostConfigured(Ok(()))`, set `rendering = Hardware`.
- Start or resume the animation that requested the boost.
- The message update causes the UI cycle to continue.

This update path occurred in the temporary probe.

Approach B: patch vendored Iced to request redraw in the configure branch.

Add this after renderer/surface replacement:

```rust
for (_id, window) in window_manager.iter_mut() {
    window.raw.request_redraw();
}
```

This would reduce dependence on callers producing meaningful app state changes. It is a small patch in code size, but it still changes vendored runtime behavior and must be reviewed and tested separately.

For the stricter "next frame must be GPU" requirement, promote Approach B from optional to required. The backend configure branch should request redraw immediately after replacing surfaces/renderers, before sending `Ok(())` back to the app. That would move responsibility for scheduling the next frame into the runtime instead of relying on app-side incidental redraws.

### Present-Boundary Handoff

The strongest implementation is a present-boundary handoff in `vendor/iced/winit/src/lib.rs`.

Current `RedrawRequested` flow:

1. update/interact UI;
2. draw into the active renderer;
3. broadcast `window::Event::RedrawRequested`;
4. call `current_compositor.present(...)`.

Current backend configure flow:

1. create a new compositor;
2. invalidate graphics caches;
3. replace renderers and surfaces;
4. send the configure result.

To target deterministic frame order, add a queued handoff mode instead of switching at an arbitrary event-loop point:

```text
app requests GPU boost
winit starts GPU preparation without replacing the active software compositor
tiny-skia remains active while preparation runs
when the GPU path is ready, winit enters CommitPending
winit requests one final software redraw if needed
software frame N is drawn and presented
after present returns Ok(()), winit replaces all renderers/surfaces
winit immediately requests redraw on all windows
GPU frame N+1 is drawn and presented
winit sends BackendBoostConfigured(Ok(()))
```

This would make the software frame that precedes handoff explicit. It is intended to prevent a stale software frame from being followed by another software redraw while the boost is pending; the strict probe must verify that behavior.

The minimal version can be implemented by adding a new runtime action in the vendored Iced backend layer, for example:

```rust
backend::Action::ConfigureAfterNextPresent(settings, sender)
```

Then in the winit loop:

- store the pending handoff action and its completion channel;
- prepare the new compositor/device/renderer state before entering `CommitPending`;
- suppress duplicate handoff requests while one is pending;
- after a successful `present_result`, run the same compositor replacement logic used by `Configure`;
- call `window.raw.request_redraw()` for every live window immediately after replacement;
- send the result through the original sender only after replacement and redraw request are complete.

This minimal action is still not enough if it creates the GPU compositor only after the final software present. That would move the wgpu stall to the commit boundary. The feasible strict version must prepare the GPU path first, then use the present boundary only for the short active-state replacement.

If the last software present fails with `SurfaceError::Lost` or `SurfaceError::Outdated`, recover the software surface first and retry the final software frame. Do not switch to GPU from a failed software present unless the failure is unrecoverable and the app chooses to treat GPU as recovery.

### Warm-Up Buffering

Normal GPU triple buffering and the CPU-to-GPU handoff buffer are different things.

Iced's current wgpu compositor configures surfaces in `vendor/iced/wgpu/src/window/compositor.rs` with:

```rust
desired_maximum_frame_latency: 1,
```

That favors low latency. Raising it to `2` or `3` can give the GPU swapchain more queue depth after the GPU is already active, but it does not let tiny-skia keep presenting while wgpu warms up. Once the app commits to the wgpu compositor, tiny-skia is no longer the active presenter.

To satisfy "if GPU lags, CPU still produces frames until GPU stabilizes," use a software-active warm-up phase:

```text
SoftwareActive:
    tiny-skia continues presenting normal frames
GpuPreparing:
    create wgpu compositor/device/renderer state without committing it
    tiny-skia remains active
GpuWarming:
    run real offscreen wgpu warm-up through Compositor::warm_up_offscreen
    tiny-skia remains active
CommitPending:
    wait for a software present boundary
    replace active compositor with warmed wgpu compositor
HardwareActive:
    first visible post-commit frame is GPU
```

The handoff buffering model is to keep at least three CPU-side frame snapshots or frame intents available while GPU preparation happens:

- `displayed_cpu_frame`: the last software frame known to be on screen;
- `next_cpu_frame`: the newest software frame produced while GPU is preparing;
- `pending_gpu_first_frame`: the first GPU frame candidate, produced only after wgpu is ready.

The app commits only when the pending GPU path is ready enough to render the next visible frame. Current strict evidence requires offscreen warm-up completion before commit and first post-commit hardware presentation. If a platform still shows visible first-frame cost despite offscreen warm-up, tiny-skia must keep presenting until the app retries or abandons the boost, and the platform must not be marked release-ready.

This requires the vendored Iced fork rather than plain `backend::configure`, because the configure path creates and commits the new compositor in one blocking operation. The implemented split is:

```text
prepare GPU compositor -> warm GPU renderer/surface -> commit at present boundary
```

The strict implementation must:

- keep software active while preparing GPU;
- allow a configurable warm-up budget, for example 2 or 3 frames or 50 ms;
- commit only after the GPU compositor can draw a frame-sized render pass without surface errors;
- if warm-up fails or times out, stay on software and mark boost as failed/deferred.

### GPU Queue Depth

After commit, consider making wgpu frame latency configurable for the first few hardware frames:

```rust
desired_maximum_frame_latency: 2 or 3
```

Use this only during `GpuWarming` or the first few `Hardware` frames, then return to the lower-latency value if possible. The goal is to absorb shader/pipeline/cache cold-start jitter without permanently adding input latency.

This is secondary to the software-active warm-up. Queue depth helps after the GPU path owns presentation; it does not solve the handoff by itself.

### Industrial Controls

This feature is now the default runtime path, so these controls must stay in place for fast rollback:

- a compile-time feature: `hybrid-rendering`, enabled by default but removable with `--no-default-features`;
- a runtime setting: `hardware_acceleration = off | lazy | diagnostic`;
- an environment override: `FRAGILE_NOTEPAD_RENDER_BACKEND=software|lazy-gpu|hardware-diagnostic`;
- an automatic fallback path that returns to software if GPU preparation, warm-up, commit, or first GPU present fails;
- a cooldown so failed GPU initialization is not retried repeatedly in one session.

The default release behavior is `lazy`, but startup first paint remains software and `FRAGILE_NOTEPAD_RENDER_BACKEND=software` remains the fast rollback path.

### Animation Start Barrier

Animation code should treat hardware boost as a barrier:

```text
animation requested
if renderer is Software:
    request prepare/warm/commit GPU handoff
    keep UI in pre-animation state
if handoff succeeds:
    start animation on the first GPU frame
if handoff fails:
    run reduced software animation or skip animation
```

Do not advance animation time during `GpuPreparing`, `GpuWarming`, or `CommitPending`. The first animation timestamp should be taken from the first redraw after `Hardware` is recorded, not from the moment the user requested the animation. This prevents the animation from appearing to jump after GPU initialization.

## Roadmap

### Phase 1: Basic Hybrid Switch

Status: complete. Fragile Notepad has a working software-to-GPU switch path. Hybrid rendering is now compiled by default, while `--no-default-features` keeps the CPU-only build path available.

Code changes:

- Add an explicit crate feature in `Cargo.toml`:

```toml
[features]
default = ["hybrid-rendering"]
hybrid-rendering = ["iced/wgpu"]
```

- Keep `iced/tiny-skia` always enabled.
- Fix or gate `examples/profile_tiny_skia_text.rs` so examples do not block hybrid builds. Prefer importing the correct headless renderer trait if that preserves the profiling tool; otherwise gate the example as tiny-skia-only and document that policy in the example header.
- Add `src/app/rendering.rs` with:
  - `RenderingState::{Software, ConfiguringHardware, Hardware, Failed}`;
  - `request_gpu_boost(&mut self) -> Task<Message>`;
  - `complete_gpu_boost(&mut self, Result<(), backend::Error>) -> Task<Message>`;
  - duplicate request guarding so repeated triggers during `ConfiguringHardware` do nothing.
- Add messages to `src/message.rs`:
  - `BackendBoostRequested`;
  - `BackendBoostConfigured(Result<(), String>)`, or a local cloneable error enum if preserving structured errors is useful.
- Route the messages in `src/app.rs` and call the rendering module from `update_inner`.
- Add a hidden/manual trigger so the path can be exercised. Acceptable first options:
  - a debug-only shortcut command;
  - an environment-driven startup task such as `FRAGILE_NOTEPAD_RENDER_BACKEND=lazy-gpu`;
  - a temporary Help or Settings diagnostic menu item that is clearly not user-facing polish.
- Keep `src/startup.rs` software-first. Do not change startup to hardware.
- Add unit tests for duplicate request state transitions if they can be expressed without running Iced's event loop.

Validation:

```powershell
cargo check
cargo check --examples
cargo check --no-default-features
```

Acceptance criteria:

- product app compiles with and without default features;
- examples policy is explicit and documented in code;
- the manual trigger calls `backend::configure` once and records `Hardware` or `Failed`;
- a failed boost leaves editing, menus, and file operations unchanged;
- hybrid rendering is the default runtime policy, and software remains the first-paint startup path.

### Phase 2: Basic Switch Observability And Redraw

Goal: make the basic switch measurable and ensure a successful configure schedules a post-switch frame immediately.

Code changes:

- Patch `vendor/iced/winit/src/lib.rs` in the existing `backend::Action::Configure` branch to request redraw for every live window after replacing renderers/surfaces and before sending `Ok(())`.
- Add a patch note under `patches/iced/fragile-notepad-iced.patch` through the normal vendor patch workflow, not as an untracked vendor-only edit.
- Add frame/backend identity trace points in the vendored Iced present path. The minimum useful event shape is:
  - window id;
  - frame sequence number;
  - backend identity (`tiny-skia`, `wgpu`, or fallback variant);
  - present result;
  - whether the frame occurred before or after configure completion.
- Add an example such as `examples/backend_switch_probe.rs` behind `hybrid-rendering`. The probe should:
  - start with `Backend::Software`;
  - wait for an initial frame;
  - request the basic boost;
  - wait for the first frame after configure;
  - print stable markers;
  - exit via `iced::exit()`.
- Add a short section to `DEVELOPMENT.md` describing how to run the probe and how to interpret its markers.

Validation:

```powershell
cargo check
cargo run --example backend_switch_probe
```

Acceptance criteria:

- the probe demonstrates an in-process switch and a post-configure frame;
- logs identify whether the post-configure frame was actually presented by `wgpu`;
- every existing window receives a redraw request after successful configure;
- if renderer identity cannot yet be proven, the probe must report that limitation instead of passing silently.

### Phase 3: Split Prepare/Warm/Commit Runtime Handoff

Goal: move "seamless" from an app-level convention to a runtime-verified property, while allowing CPU presentation to continue until GPU is stable.

Status: implemented in vendored Iced for the strict probe path. The action prepares the wgpu path while software remains active, runs offscreen warm-up, commits at a present boundary, and emits structured strict handoff results. The current limitation is validation coverage, not absence of the core runtime path.

Code changes:

- Add a new vendored Iced runtime action, `backend::Action::PrepareWarmAndCommit(settings, sender)`. Keep the existing `Configure` action intact for simple switches.
- Add pending handoff state to `vendor/iced/winit/src/lib.rs`. The state stores:
  - requested backend settings;
  - result sender;
  - prepared compositor or preparation future state;
  - per-window pending renderer/surface state if surfaces are created before commit;
  - handoff phase (`Preparing`, `Warming`, `CommitPending`);
  - timeout/failure reason.
- Keep the active `window::Manager` rendering through tiny-skia while the pending GPU path prepares.
- Prepare wgpu device/compositor resources without replacing active window renderers.
- Warm the GPU path with real offscreen wgpu render work through `Compositor::warm_up_offscreen`. tiny-skia returns `Unsupported`; strict success requires wgpu warm-up evidence.
- Queue commit until a successful software present boundary. After that present:
  - invalidate graphics caches;
  - replace active compositor/renderers/surfaces;
  - request redraw on every live window;
  - send success only after redraw is requested.
- Add cancellation handling for close/exit while `Preparing`, `Warming`, or `CommitPending`.
- Update `src/app/rendering.rs` to use the new action under `hybrid-rendering`, and keep the Phase 1 basic configure path available for diagnostic comparison.

Validation:

```powershell
cargo check
$env:FRAGILE_PERF_TRACE='1'
cargo run --example backend_switch_probe -- --scenario=single-window
```

Acceptance criteria:

- instrumentation shows CPU/tiny-skia continues presenting while GPU preparation is slow;
- after `CommitPending` starts, at most one final software frame is presented;
- the first frame presented after successful replacement uses the GPU renderer;
- every live window receives a redraw request immediately after replacement;
- trace evidence includes `backend_handoff_warm_complete` with wgpu renderer family and completed submission;
- failed GPU creation leaves the last software frame on screen and returns to `Software` or `Failed` without a blank frame.

### Phase 4: Failure Injection, Rollback, And Controls

Goal: make the hybrid path recoverable and controllable before it is connected to user-visible animation.

Code changes:

- Add runtime controls:
  - `FRAGILE_NOTEPAD_RENDER_BACKEND=software|lazy-gpu|hardware-diagnostic`;
  - an `EditorSettings` field such as `hardware_acceleration = off | lazy | diagnostic`;
  - settings XML persistence and parsing for that field;
  - a Settings UI control only if the feature is meant to be user-visible in diagnostic builds.
- Add rollback state in `src/app/rendering.rs`:
  - failed prepare;
  - failed warm-up;
  - failed commit;
  - first GPU present failure;
  - cooldown until next retry.
- Add failure injection switches behind environment variables or a test-only cfg:
  - fail GPU prepare;
  - delay GPU prepare;
  - fail first GPU present;
  - fail after commit and require software rollback.
- Extend the vendored Iced runtime action to return structured failure categories that the app can map to status/log messages.
- Add tests for settings parsing, environment override precedence, duplicate retry suppression, and cooldown behavior.
- Extend `examples/backend_switch_probe.rs` with failure modes and stable marker output.

Acceptance criteria:

- users and diagnostic runs can force software;
- GPU failures do not loop or repeatedly retry in one session;
- injected prepare/warm/commit/present failures leave the editor usable;
- status/log output distinguishes unavailable GPU from transient handoff failure;
- rollback releases pending GPU resources.

### Phase 5: First Animation Consumer

Goal: connect GPU boost to a real UI reason.

Code changes:

- Pick one contained animation surface. Prefer a panel or overlay because it is easier to pause before the first animation frame than the core editor.
- Add animation state local to the relevant UI/app module, for example:
  - `PanelAnimation::{Idle, WaitingForHardware, Running { started_at }, Disabled}`;
  - no animation clock advancement while hardware is pending;
  - first animation timestamp captured only after `RenderingState::Hardware`.
- Before starting the animation, dispatch a domain message that requests the boost if the renderer is still software.
- Add a fallback path:
  - reduced or skipped animation when boost fails;
  - no visible broken intermediate state;
  - no repeated boost attempts for the same animation after failure.
- Add tests for the animation state machine if it is app-level state. If it stays widget-local, add focused tests around time/barrier helpers where practical.
- Update the probe or add a small manual diagnostic path that confirms no animation frame is emitted before the hardware barrier opens.

Good first candidates:

- panel open/close transition;
- transient overlay fade;
- tab drag/drop affordance;
- search/settings window polish.

Avoid using the main editor caret blink as the first consumer. It already has redraw scheduling and does not need GPU.

Validation:

```powershell
cargo test
cargo run --example backend_switch_probe
```

Acceptance criteria:

- animation feature works after software startup;
- no animation frame is presented on software after boost begins;
- failed GPU boost leaves the feature usable;
- no editor input latency regression is visible during normal typing.

### Phase 6: Multi-Window, Lifecycle, And Telemetry Hardening

Goal: make the implementation robust across the app's real window model and lifecycle events, then produce enough evidence to decide whether the feature is release-ready.

Status: partially implemented and not release-complete. Current Windows strict validation has passing single-window and multi-window evidence with Wgpu/Vulkan presentation, non-null warm evidence, and strict outcome success. Windows close-during-preparing also reports `result=ok` for intentional `Cancelled + Preparing + NotNeeded` cancellation. Cross-platform strict proof is still blocked by WSL/Linux GPU adapter creation and macOS prototype-only status; IME coverage still has a residual production-hook gap for private `handle_event` dispatch.

Code changes:

- Log boost attempts, success, failure, and duration.
- Measure the one-time configure stall.
- Add a user setting or environment override:
  - force software;
  - allow lazy boost;
  - force hardware at startup for diagnostics only.
- Consider a cooldown after failure so the app does not retry repeatedly.
- Extend the handoff runtime to include every managed window:
  - main editor window;
  - settings window;
  - advanced search window;
  - windows opened during `Preparing` or `CommitPending`;
  - windows closed during `Preparing` or `CommitPending`.
- Ensure resized windows get correctly sized replacement surfaces and do not reuse stale physical sizes.
- Preserve IME/preedit state across renderer replacement or explicitly clear/rebuild it in a controlled way.
- Measure the one-time configure/prepare/warm/commit stall and expose it through trace output.
- Add trace events for:
  - software present start/end;
  - GPU prepare start/end;
  - GPU warm-up start/end;
  - offscreen warm-up completion/failure evidence;
  - commit start/end;
  - first GPU present start/end;
  - rollback reason.
- Add renderer identity to frame traces:
  - `backend=tiny-skia`;
  - `backend=wgpu`;
  - adapter name/backend when hardware is active.
- Add a lightweight results log format for platform runs, for example `target/hybrid-rendering-probes/*.json`, containing backend, OS, adapter, phase timings, warm-up evidence, strict outcome, and pass/fail reason.
- Add automated probes:
  - successful software-to-GPU handoff;
  - forced GPU creation failure;
  - forced first GPU present failure;
  - repeated boost request de-duplication;
  - multi-window handoff with settings/search windows open;
  - resize during `GpuPreparing`;
  - close/exit during `GpuPreparing` and `CommitPending`.
- Run a platform matrix:
  - Windows with DirectX 12;
  - WSL/Linux with an explicit GUI/GPU-capable environment before treating Linux results as meaningful;
  - Linux X11;
  - Linux Wayland;
  - software-only or GPU-denied environment;
  - at least one low-end/integrated GPU.
  - macOS prototype-only if available; do not claim native macOS validation from this branch until it has been run and recorded on macOS hardware.

Acceptance criteria:

- a failed adapter/device creation is visible in logs;
- the app never loops on repeated GPU initialization failures;
- users can opt out if a driver is problematic;
- no observed blank frame in visual/manual probe on the tested platform matrix;
- no observed app-window recreation;
- no crash or panic on GPU failure;
- first animation frame starts only after first GPU present;
- software remains usable during GPU preparation;
- rollback leaves editor input, selection, IME, menus, and secondary windows usable;
- memory growth from pending GPU resources is bounded and released after rollback or successful commit.

### Release Gates

Hybrid rendering is enabled by default only with rollback controls active. Strict seamless animation claims still require these gates:

- `cargo check`, `cargo test`, and examples policy pass with default features and with `--no-default-features`;
- the single-window strict backend switch probe verifies:
  - at least one software frame during GPU preparation;
  - one final software frame at commit boundary;
  - `backend_handoff_warm_complete` appears before `backend_handoff_commit_pending`;
  - warm-up evidence reports `renderer_family=Wgpu` and `submission_completed=true`;
  - first post-commit presented frame is GPU;
  - no software animation frame after commit begins;
- GPU failure injection verifies rollback to software;
- Windows single-window and multi-window strict probes record `result=ok`, strict outcome success, Wgpu/Vulkan presented evidence, and non-null warm evidence;
- resize/close during handoff does not crash;
- WSL/Linux validation records strict trace/result JSON only after GPU adapter creation succeeds; current `GraphicsAdapterNotFound` / no suitable adapter results are environment blockers;
- native macOS remains prototype-only until strict trace/result JSON is recorded on macOS hardware;
- manual smoke test shows no visible flash on supported platforms.

If a platform gate fails, use the runtime setting or `FRAGILE_NOTEPAD_RENDER_BACKEND=software` to force CPU-only rendering on that platform while the issue is investigated.

## Risks And Mitigations

### One-Time Stall

Iced uses a blocking compositor creation path during backend configure. The UI may pause briefly during the switch.

Mitigation:

- split GPU setup from commit so software keeps presenting while GPU setup runs;
- keep a final software frame on screen only during the short commit boundary;
- trigger before the animation becomes visible;
- keep the first animation frame delayed until the first GPU redraw;
- measure and log duration before deciding whether to prewarm earlier.

This requires a true prepare/commit split: create the wgpu device/compositor before handoff, keep rendering with tiny-skia until preparation succeeds, then do a short present-boundary commit. That is a larger Iced fork because the current public `backend::configure` action combines preparation and commit.

### Renderer Type Changes

When `wgpu` and `tiny-skia` are both enabled, `iced::Renderer` can become a fallback renderer instead of the concrete tiny-skia renderer expected by profiling examples.

Mitigation:

- avoid app code that relies on concrete renderer constructors;
- isolate tiny-skia-specific profiling examples;
- compile the product with default features and with `--no-default-features` in validation.

### Redraw After Configure

The current Iced configure branch swaps renderers and surfaces but does not explicitly request redraw in that branch.

Mitigation:

- add immediate redraw requests in the backend configure branch;
- prefer a present-boundary handoff so the redraw follows the final software present;
- always map the configure result into an app message that changes state;
- begin/resume animation only after the success message;
- verify with a probe that the first post-handoff presented frame is GPU.

### Cache Invalidation

Iced invalidates graphics caches during backend configure. This is correct, but the first hardware frame may need to rebuild cached geometry/text/image resources.

Mitigation:

- expect the first hardware frame to be more expensive without warm-up;
- use the GPU warm-up phase to populate pipelines/caches before visible commit where possible;
- switch before the user-visible animation starts;
- avoid switching repeatedly.

### Added Latency From GPU Queue Depth

Increasing wgpu frame latency to 2 or 3 can smooth cold hardware frames, but it can also add input latency if left enabled permanently.

Mitigation:

- only increase queue depth during `GpuWarming` or the first few `HardwareActive` frames;
- return to the default low-latency configuration after the GPU path is stable;
- measure typing and selection latency before keeping any higher latency setting.

## Recommended First PR

The basic-switch and strict runtime warm-up work are complete enough for diagnostic validation. The next PRs should stay focused on cross-platform evidence, WSL/Linux GPU adapter availability, and any remaining animation consumer work.

Completed baseline:

1. Add `default = ["hybrid-rendering"]` and keep startup software-first.
2. Keep examples compiling under the default hybrid feature set.
3. Add `src/app/rendering.rs`, rendering messages, and a hidden/manual boost trigger that uses `backend::configure`.
4. Add duplicate-request and failure-state tests around the app rendering state.
5. Patch vendored Iced to request redraw after a successful basic configure, then export the vendor patch.
6. Add `examples/backend_switch_probe.rs` with switch and failure markers.
7. Add the strict prepare/warm/commit runtime action with real wgpu offscreen warm-up and structured evidence.
8. Tighten probe gates so `result=ok` requires warm-up and frame-order trace evidence.

Still future work:

- user-facing animation;
- WSL/Linux strict hardware proof after GPU adapter creation succeeds;
- broader platform matrix evidence beyond current Windows strict runs;
- native macOS validation if macOS is ever promoted beyond prototype-only.

Do not add a polished animation before the runtime architecture, probes, rollback, and kill switch pass. The animation is the consumer, not the proof.

## Validation Commands

Run these from `fragile-notepad/`:

```powershell
cargo check
cargo test
cargo check --examples
cargo check --no-default-features
$env:FRAGILE_PERF_TRACE='1'
cargo run --example backend_switch_probe -- --scenario=single-window
cargo run --example backend_switch_probe -- --scenario=multi-window
```

Expected current baseline:

- default builds include `hybrid-rendering`;
- `--no-default-features` keeps the CPU-only path compiling;
- the Windows single-window strict probe reports `result=ok` with strict outcome success, Wgpu/Vulkan presented evidence, and non-null `warm_complete_us`;
- the Windows multi-window strict probe reports `result=ok` with strict outcome success, Wgpu/Vulkan presented evidence for both live windows, and non-null warm evidence;
- the Windows close-during-preparing probe reports `result=ok` for intentional `Cancelled + Preparing + NotNeeded` cancellation;
- injected prepare/warm/commit/first-present failures are observable through stable probe markers;
- result JSON and trace CSV must be recorded for release validation;
- with `FRAGILE_PERF_TRACE=1` and no explicit `FRAGILE_PERF_TRACE_DIR`, `backend_switch_probe` writes traces under `CARGO_TARGET_DIR/perf/<scenario>/<mode>-<failure>/fragile-perf.csv`;
- WSL/Linux compile checks and targeted startup/lifecycle tests pass outside the sandbox, but strict hardware proof is currently blocked by GPU adapter creation (`GraphicsAdapterNotFound` / no suitable adapter);
- Cargo commands may emit vendored `encoding_rs` lifetime syntax warnings;
- native macOS is prototype-only and not locally validated;
- the stricter platform matrix is still required before claiming release-ready seamless animation handoff.
