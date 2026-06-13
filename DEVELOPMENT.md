# Development

## Vendored Dependencies

Fragile Notepad builds against Git checkouts under `vendor/`. Each vendor
directory is managed by the setup scripts and ignored by Git. Project-owned
changes are stored as patch files under `patches/`, with the upstream base
recorded in `BASE_REVISION`.

Current vendors:

- `vendor/iced`
  - remote: `https://github.com/iced-rs/iced.git`
  - base: `patches/iced/BASE_REVISION`
  - patch: `patches/iced/fragile-notepad-iced.patch`
  - extra apply config: `core.autocrlf=true`
- `vendor/encoding_rs`
  - remote: `https://github.com/hsivonen/encoding_rs.git`
  - base: `patches/encoding_rs/BASE_REVISION`
  - patch: `patches/encoding_rs/oem-code-pages.patch`

## Clone Setup

After cloning, bootstrap and patch the vendor checkouts:

```powershell
.\scripts\setup-vendor.ps1 apply
```

On Linux or macOS:

```bash
bash scripts/setup-vendor.sh apply
```

## Patch Workflow

Run the setup wrapper from the Fragile Notepad repo root for routine patch
application and status checks:

```powershell
.\scripts\setup-vendor.ps1 status
.\scripts\setup-vendor.ps1 apply
.\scripts\setup-vendor.ps1 update
```

```bash
bash scripts/setup-vendor.sh status
bash scripts/setup-vendor.sh apply
bash scripts/setup-vendor.sh update
```

Use `update` to move both vendor checkouts to the latest configured upstream
branches and refresh their patches:

- `vendor/iced`: `https://github.com/iced-rs/iced.git`, branch `master`
- `vendor/encoding_rs`: `https://github.com/hsivonen/encoding_rs.git`, branch
  `main`

The lower-level patch helper is still used when exporting local vendor changes
or refreshing a vendor base revision.

PowerShell:

```powershell
.\scripts\vendor-patch.ps1 status -VendorDir vendor/iced -Patch patches/iced/fragile-notepad-iced.patch -BaseRevisionFile patches/iced/BASE_REVISION -GitVendor -GitConfig core.autocrlf=true
.\scripts\vendor-patch.ps1 apply -VendorDir vendor/iced -Patch patches/iced/fragile-notepad-iced.patch -BaseRevisionFile patches/iced/BASE_REVISION -GitVendor -GitConfig core.autocrlf=true
.\scripts\vendor-patch.ps1 export -VendorDir vendor/iced -Patch patches/iced/fragile-notepad-iced.patch -BaseRevisionFile patches/iced/BASE_REVISION -GitVendor -GitConfig core.autocrlf=true
.\scripts\vendor-patch.ps1 refresh -VendorDir vendor/iced -Patch patches/iced/fragile-notepad-iced.patch -BaseRevisionFile patches/iced/BASE_REVISION -GitVendor -GitConfig core.autocrlf=true -Remote https://github.com/iced-rs/iced.git -Revision <upstream-iced-commit>

.\scripts\vendor-patch.ps1 status -VendorDir vendor/encoding_rs -Patch patches/encoding_rs/oem-code-pages.patch -BaseRevisionFile patches/encoding_rs/BASE_REVISION -GitVendor
.\scripts\vendor-patch.ps1 apply -VendorDir vendor/encoding_rs -Patch patches/encoding_rs/oem-code-pages.patch -BaseRevisionFile patches/encoding_rs/BASE_REVISION -GitVendor
.\scripts\vendor-patch.ps1 export -VendorDir vendor/encoding_rs -Patch patches/encoding_rs/oem-code-pages.patch -BaseRevisionFile patches/encoding_rs/BASE_REVISION -GitVendor
.\scripts\vendor-patch.ps1 refresh -VendorDir vendor/encoding_rs -Patch patches/encoding_rs/oem-code-pages.patch -BaseRevisionFile patches/encoding_rs/BASE_REVISION -GitVendor -Remote https://github.com/hsivonen/encoding_rs.git -Revision <upstream-encoding-rs-commit>
```

Bash:

```bash
bash scripts/vendor-patch.sh status --vendor-dir vendor/iced --patch patches/iced/fragile-notepad-iced.patch --base-revision-file patches/iced/BASE_REVISION --git-vendor --git-config core.autocrlf=true
bash scripts/vendor-patch.sh apply --vendor-dir vendor/iced --patch patches/iced/fragile-notepad-iced.patch --base-revision-file patches/iced/BASE_REVISION --git-vendor --git-config core.autocrlf=true
bash scripts/vendor-patch.sh export --vendor-dir vendor/iced --patch patches/iced/fragile-notepad-iced.patch --base-revision-file patches/iced/BASE_REVISION --git-vendor --git-config core.autocrlf=true
bash scripts/vendor-patch.sh refresh --vendor-dir vendor/iced --patch patches/iced/fragile-notepad-iced.patch --base-revision-file patches/iced/BASE_REVISION --git-vendor --git-config core.autocrlf=true --remote https://github.com/iced-rs/iced.git --revision <upstream-iced-commit>

bash scripts/vendor-patch.sh status --vendor-dir vendor/encoding_rs --patch patches/encoding_rs/oem-code-pages.patch --base-revision-file patches/encoding_rs/BASE_REVISION --git-vendor
bash scripts/vendor-patch.sh apply --vendor-dir vendor/encoding_rs --patch patches/encoding_rs/oem-code-pages.patch --base-revision-file patches/encoding_rs/BASE_REVISION --git-vendor
bash scripts/vendor-patch.sh export --vendor-dir vendor/encoding_rs --patch patches/encoding_rs/oem-code-pages.patch --base-revision-file patches/encoding_rs/BASE_REVISION --git-vendor
bash scripts/vendor-patch.sh refresh --vendor-dir vendor/encoding_rs --patch patches/encoding_rs/oem-code-pages.patch --base-revision-file patches/encoding_rs/BASE_REVISION --git-vendor --remote https://github.com/hsivonen/encoding_rs.git --revision <upstream-encoding-rs-commit>
```

Command behavior:

- `status` prints the recorded base revision, patch path, vendor HEAD, and
  current vendor changes. It reports whether the recorded base revision matches
  the vendor HEAD. The setup wrapper keeps `status` read-only; missing vendor
  directories are reported without cloning.
- `apply` clones missing vendor checkouts, checks out the recorded base
  revision, and applies the patch to a clean vendor checkout.
- `export` refreshes the patch from the current vendor working tree and updates
  `BASE_REVISION` to the vendor HEAD. Untracked vendor files are included as
  new-file patch hunks automatically. Existing non-empty patch files are backed
  up under `patches/**/.backups/` before replacement, and empty exports refuse
  to overwrite non-empty patches unless `-Force` / `--force` is used. After an
  export, stage the patch artifacts so fresh clones can reproduce the same
  vendor base:

```powershell
git add patches/iced/BASE_REVISION patches/iced/fragile-notepad-iced.patch
git add patches/encoding_rs/BASE_REVISION patches/encoding_rs/oem-code-pages.patch
```

```bash
git add patches/iced/BASE_REVISION patches/iced/fragile-notepad-iced.patch
git add patches/encoding_rs/BASE_REVISION patches/encoding_rs/oem-code-pages.patch
```

  Re-run `.\scripts\setup-vendor.ps1 status` or `bash scripts/setup-vendor.sh
  status` before committing; each vendor should report `pin status: ok`.
- `refresh` optionally fetches a remote, checks out the requested upstream
  revision, records it as the new base, and reapplies the project patch.
- `update` exports the current patch, reverses it to return to a clean upstream
  base, fetches a configured upstream branch, applies the project patch on top,
  then exports the refreshed patch and `BASE_REVISION`. On a fresh clone, the
  setup wrapper bootstraps missing vendor checkouts before running `update`; it
  does not reset already-present vendor directories.

The script refuses to apply over dirty vendor changes unless `-Force` /
`--force` is passed. Review vendor status before exporting so unrelated local
experiments do not enter project patches.

## Validation

Generated raw RGBA icon files are not tracked. Rebuild them after changing SVG
or PNG icon sources:

```powershell
.\scripts\generate_icon_assets.ps1
```

On Linux or macOS:

```bash
bash scripts/generate_icon_assets.sh
```

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

Before handing off changes that touch editor rendering or a vendored patch, run:

```powershell
cargo check
cargo test
cargo check --examples
cargo check --no-default-features
```

## Packaging

Release packaging expectations are documented in `PACKAGING.md`. In short:
apply vendor patches, regenerate icon assets when sources change, run the CI
entry point, and then build the release binary.

For renderer performance changes, also run:

```powershell
cargo run --release --example profile_tiny_skia_text
cargo run --release --example profile_render
```

## Backend Switch Probe

The backend switch probe verifies the runtime path for starting with the
software renderer, requesting the prepare/warm/commit backend handoff to
`Backend::Hardware(Api::Best)`, running real wgpu offscreen warm-up, observing
strict frame-order evidence, and exiting. Use trace collection for strict
validation; without it, the probe may report `indeterminate` because strict
success requires warm-up and present-order evidence:

```powershell
$env:CARGO_TARGET_DIR='target-codex-check'
$env:FRAGILE_PERF_TRACE='1'
cargo run --example backend_switch_probe -- --scenario=single-window
```

On Windows, keep the generated result JSON and trace CSV from the run. Current
Windows strict validation records `result=ok`, strict outcome success,
Wgpu/Vulkan presented evidence, and non-null warm evidence for both
single-window and multi-window scenarios. For WSL/Linux, compile checks and
targeted startup/lifecycle tests pass outside the sandbox and GUI prerequisites
are present, but strict hardware proof is currently blocked by GPU adapter
creation (`GraphicsAdapterNotFound` / no suitable adapter). Treat that as an
environment blocker, not source pass evidence. Native macOS is prototype-only
for this branch and has not been locally validated.

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
