# Seamless Hybrid Rendering Review

Date: 2026-06-13

Scope: Fresh review of the current working tree against `SEAMLESS_HYBRID_RENDERING.md`. This review intentionally ignores earlier generated review notes and evaluates only the current implementation.

## Executive Summary

The implementation has a working software-first hybrid rendering path and has made substantial progress toward strict handoff validation. The app starts on tiny-skia, default builds include the hybrid feature, app-level rendering policy/state exists, failure categories are structured, rollback is retained for first-present failure, and the single-window strict probe can prove a first `wgpu` present.

`SEAMLESS_HYBRID_RENDERING.md` is not fully complete. The main remaining gaps are release-gate issues: the `Warming` phase is not real offscreen GPU warm-up, strict probe success does not enforce all frame-order trace gates, the multi-window strict probe can hang, the exported Iced patch is stale, and Phase 6 platform/app-lifecycle evidence is incomplete.

## Phase Completion Status

| Phase | Status | Notes |
| --- | --- | --- |
| Phase 1: Basic Hybrid Switch | Complete | Hybrid rendering is enabled by default, software-first startup remains, app rendering state/messages exist, settings/env controls exist, and build gates pass. |
| Phase 2: Basic Switch Observability And Redraw | Mostly complete | Redraw-after-configure and renderer identity evidence exist. Strict trace evidence is available when configured, but not enforced for all success paths. |
| Phase 3: Prepare/Warm/Commit Runtime Handoff | Partial | Async prepare, present-boundary commit, first-present evidence, and rollback retention exist. Real offscreen GPU warm-up is not implemented. |
| Phase 4: Failure Injection, Rollback, And Controls | Partial | Failure injection and structured categories exist, and injected first-present failure restores rollback. Broader resource-release/platform proof remains incomplete. |
| Phase 5: First Animation Consumer | Partial | About dialog animation uses a hardware barrier and has tests. Runtime validation that no software animation frame is emitted after boost begins is not fully release-proven. |
| Phase 6: Multi-Window, Lifecycle, And Telemetry | Not complete | Generic probe scaffolding exists, but app-specific settings/search window coverage, IME/preedit handoff evidence, robust multi-window probe completion, and platform matrix evidence are missing. |

## Findings

### 1. Major: `Warming` Is Not Real GPU Warm-Up

`prepare_warm_and_commit` explicitly states that it does not imply full offscreen warm-up unless the backend implements it internally. In `iced_winit`, the runtime enters `Warming`, emits the warm marker, checks only injected warm failure, and then moves directly to `CommitPending`. Renderer and visible surface creation still happen at commit.

Evidence:

- `vendor/iced/runtime/src/backend.rs:33` documents that full offscreen warm-up is not implied.
- `vendor/iced/winit/src/lib.rs:1838` enters `StrictHandoffPhase::Warming`.
- `vendor/iced/winit/src/lib.rs:1874` checks only injected warm failure.
- `vendor/iced/winit/src/lib.rs:1892` transitions to `CommitPending`.
- `vendor/iced/winit/src/lib.rs:1769` creates the renderer during commit.
- `vendor/iced/winit/src/lib.rs:1770` creates the visible surface during commit.

Impact: The strict seamless claim still cannot rely on the warm-up phase to prove that cold GPU renderer/surface cost has been paid before visible handoff.

Suggested fix: Implement measurable offscreen warm-up, or rename/document this path as prepare/commit and keep strict seamless claims gated behind measured first-frame cost.

### 2. Major: Strict Probe Success Does Not Enforce All Frame-Order Release Gates

The probe computes trace-derived evidence for software frames during prepare, software frames after commit pending, and first post-commit backend. However, `final_result_from_strict_result` accepts success based on strict outcome and per-window `RendererFamily::Wgpu` present evidence; it does not require those trace-derived frame-order checks to pass.

Evidence:

- `examples/backend_switch_probe.rs:654` starts strict result classification.
- `examples/backend_switch_probe.rs:697` validates per-window strict outcome evidence.
- `examples/backend_switch_probe.rs:1148` computes `software_frames_during_prepare`.
- `examples/backend_switch_probe.rs:1168` computes `software_frames_after_commit_pending`.
- `examples/backend_switch_probe.rs:1183` computes `first_post_commit_backend`.

Impact: A probe can report `result=ok` while frame-order trace evidence is missing or incomplete. This is weaker than the release gates in `SEAMLESS_HYBRID_RENDERING.md`.

Suggested fix: In strict mode, require trace evidence for the release gates or return `indeterminate` when it is missing.

### 3. Major: Multi-Window Strict Probe Can Hang

The multi-window probe opened both windows and began the strict switch in local validation, but it did not finish within the 120 second command timeout. The probe timeout logic covers initial frame and post-configure frame waiting, but strict prepare/warm/commit waits in `Switching` until `StrictHandoffCompleted` arrives.

Evidence:

- `examples/backend_switch_probe.rs:135` defines `ProbeState::Switching`.
- `examples/backend_switch_probe.rs:285` handles `StrictHandoffCompleted`.
- `examples/backend_switch_probe.rs:485` starts timeout handling.
- `examples/backend_switch_probe.rs:502` only times out `WaitingPostSwitchFrame`, not strict `Switching`.

Observed validation output:

```text
BACKEND_SWITCH_PROBE_START initial_backend=software mode=prepare_warm_commit failure=none scenario=multi-window
BACKEND_SWITCH_PROBE_WINDOW_OPENED id=Id(1) count=1
BACKEND_SWITCH_PROBE_WINDOW_OPENED id=Id(2) count=2
BACKEND_SWITCH_PROBE_INITIAL_FRAME window=unscoped frame=3 elapsed_ms=157
```

The command then timed out externally after 120 seconds.

Impact: Phase 6 multi-window evidence is not reliable if the probe can hang instead of producing a bounded failed or indeterminate result.

Suggested fix: Add a strict handoff timeout for `Switching` and include the last observed handoff phase/evidence in the result log.

### 4. Major: Exported Iced Patch Is Stale

The vendored Iced tree contains structured strict handoff outcomes and retained rollback state, but `patches/iced/fragile-notepad-iced.patch` still includes older handoff code paths with unstructured success.

Evidence:

- `vendor/iced/core/src/backend.rs:169` defines `StrictHandoffOutcome`.
- `vendor/iced/winit/src/lib.rs:489` defines retained rollback state.
- `patches/iced/fragile-notepad-iced.patch:5076` still includes an older `sender.send(Ok(()))` handoff completion path.

Impact: Reapplying the exported patch may not recreate the current vendored implementation. This breaks the documented vendor patch workflow.

Suggested fix: Regenerate `patches/iced/fragile-notepad-iced.patch` from the current `vendor/iced` changes.

### 5. Major: Phase 6 App-Specific Lifecycle And Platform Evidence Is Incomplete

The document requires settings/search windows, windows opened during handoff, IME/preedit preservation, platform matrix runs, and result logs with backend/adapter/timing/pass-fail information. The current probe opens generic probe windows, result logs are local-run artifacts, and there is no app-specific handoff coverage for settings/search or IME/preedit preservation.

Evidence:

- `SEAMLESS_HYBRID_RENDERING.md:821` requires main/settings/search window coverage.
- `SEAMLESS_HYBRID_RENDERING.md:828` requires IME/preedit preservation or controlled rebuild evidence.
- `SEAMLESS_HYBRID_RENDERING.md:841` requires result logs with backend, OS, adapter, phase timings, and pass/fail reason.
- `SEAMLESS_HYBRID_RENDERING.md:850` requires a platform matrix.
- `examples/backend_switch_probe.rs:179` opens generic probe windows.
- `examples/backend_switch_probe.rs:832` writes the current JSON schema.

Impact: The repo does not yet contain the release evidence needed to claim strict seamless behavior across the supported matrix.

Suggested fix: Add app-level probe scenarios for settings/search windows and IME/preedit during handoff, extend result metadata with matrix slot and adapter/backend data, and record platform runs.

### 6. Minor: CPU-Only Build Has Warning Noise

`cargo check --no-default-features` passes, but `iced_winit` emits unused warnings around wgpu-gated handoff helpers.

Evidence from validation:

- unused import: `crate::futures::futures::stream`
- unused constant: `RENDER_INJECT_PREPARE_DELAY_MS_ENV`
- unused function: `maybe_delay_backend_prepare`
- unused function: `maybe_delay_backend_prepare_async`

Impact: This is not a functional failure, but it weakens the CPU-only validation gate if warnings are later promoted.

Suggested fix: Gate these imports/constants/functions behind the same `feature = "wgpu"` condition as `PrepareWarmAndCommit`.

## Implemented Strengths To Preserve

- Software-first startup remains in place.
- Default features include hybrid rendering while `--no-default-features` still compiles.
- App rendering state, messages, and settings/env controls are clear and testable.
- Strict handoff returns structured success/error data.
- First-present success includes per-window renderer-family evidence.
- Injected first-present failure restores the retained previous backend.
- Single-window strict probe reports `wgpu` first-present evidence.
- About dialog animation waits for the hardware barrier and has focused tests.

## Validation Run

Commands run:

```powershell
$env:CARGO_TARGET_DIR='target-codex-review'; cargo test
$env:CARGO_TARGET_DIR='target-codex-review'; cargo check --examples
$env:CARGO_TARGET_DIR='target-codex-review-no-default'; cargo check --no-default-features
$env:CARGO_TARGET_DIR='target-codex-review'; $env:FRAGILE_PERF_TRACE='1'; $env:FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR='target-codex-review-probes'; cargo run --example backend_switch_probe -- --scenario=single-window
$env:CARGO_TARGET_DIR='target-codex-review'; $env:FRAGILE_PERF_TRACE='1'; $env:FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR='target-codex-review-probes'; cargo run --example backend_switch_probe -- --fail=first-present
$env:CARGO_TARGET_DIR='target-codex-review'; $env:FRAGILE_PERF_TRACE='1'; $env:FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR='target-codex-review-probes'; cargo run --example backend_switch_probe -- --scenario=multi-window
```

Results:

- `cargo test`: passed.
- `cargo check --examples`: passed.
- `cargo check --no-default-features`: passed with warnings described above.
- `backend_switch_probe --scenario=single-window`: passed with `wgpu` first-present evidence.
- `backend_switch_probe --fail=first-present`: passed with `first_present` failure and rollback `restored`.
- `backend_switch_probe --scenario=multi-window`: timed out externally after 120 seconds.

Trace-backed single-window result showed:

```text
software_frames_during_prepare=458
software_frames_after_commit_pending=1
first_post_commit_backend=Vulkan
```

This is useful local evidence for the single-window case, but it is not yet enforced as a strict success gate and does not replace the Phase 6 platform matrix.

## Recommended Priority Order

1. Make strict probe success enforce the frame-order release gates or return `indeterminate`.
2. Add a timeout/result path for strict `Switching`, then fix the multi-window hang.
3. Implement real warm-up or explicitly downgrade the current `Warming` phase to a naming/telemetry scaffold.
4. Regenerate the exported Iced patch.
5. Add app-specific settings/search/IME lifecycle probes.
6. Extend result logs and record the platform matrix.
7. Clean up `--no-default-features` warnings.
