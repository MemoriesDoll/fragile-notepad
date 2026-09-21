# Packaging

Fragile Notepad is packaged from a source tree that includes patched vendored
dependencies and generated embedded icon assets. A release build is not just
`cargo build --release`; the vendor and asset preparation steps are part of the
package contract.

## Source Layout

- `src/` contains the application.
- `assets/` contains source assets and generated RGBA icon files consumed at
  compile time through `src/assets.rs`.
- `vendor/iced` and `vendor/encoding_rs` are script-managed vendor checkouts
  pinned by `patches/*/BASE_REVISION`.
- `patches/` contains project-owned changes applied on top of vendor bases.
- `scripts/` contains repeatable setup, asset generation, and CI entry points.

## Preparing a Checkout

Run the vendor setup before building from a fresh clone:

Use the Rust stable toolchain and Python with Pillow installed (the same asset
tooling used by `.github/workflows/ci.yml`). Platform windowing dependencies must
also be available; the Linux CI job lists the required X11/Wayland packages.

```powershell
.\scripts\setup-vendor.ps1 apply
```

On Linux or macOS:

```bash
bash scripts/setup-vendor.sh apply
```

## Generated Assets

RGBA icon files are generated from the tracked SVG sources. Regenerate them
on a fresh checkout and after changing any icon source; the generated files are
ignored by Git:

```powershell
.\scripts\generate_icon_assets.ps1
```

On Linux or macOS:

```bash
bash scripts/generate_icon_assets.sh
```

UI icon embedding is owned by `src/ui/icons/`; non-UI assets are owned by
`src/assets.rs`. See `assets/icons/README.md` for the source inventory and review
gallery. Original colored artwork lives in `assets/icons/colored/`.

Distribute the project `LICENSE` and `assets/icons/NOTICE.txt` (renamed to
`ICON-NOTICES.txt`) beside the binary. The nightly archive jobs include both;
manual packages must do the same to preserve the icon copyright notices.
The colored artwork is all rights reserved and excluded from the project's
BSD-3-Clause license; its full notice is included in `ICON-NOTICES.txt`.

## Local Validation

The standard local validation entry point is:

```powershell
.\scripts\ci.ps1
```

On Linux or macOS:

```bash
bash scripts/ci.sh
```

The CI scripts run formatting, asset generation, and the following Cargo checks.
For a manual equivalent, run:

```powershell
cargo check
cargo test
cargo check --examples
cargo check --no-default-features
```

Vendored package regression tests and live backend-switch scenarios are additional
checks, not part of these scripts. Commands and environment requirements are in
[DEVELOPMENT.md](DEVELOPMENT.md). On Linux, `scripts/ci.sh` uses `xvfb-run` when
available. Both GitHub workflows install Mesa Lavapipe and select its Vulkan ICD
with `scripts/setup-ci-vulkan.sh`, which runs `vulkaninfo --summary` before
compilation. Linux GUI tests run under Xvfb, including the nightly release gates.
This exercises wgpu on a software Vulkan device; it does not validate physical
GPU drivers. The icon parity test requires an available wgpu adapter in CI unless
`FRAGILE_ALLOW_WGPU_PARITY_SKIP=1` explicitly opts out; skipped GPU checks are not
GPU-validation evidence.

## Release Build

After vendor setup, asset generation, and validation:

```powershell
cargo build --release --locked
```

The resulting binary is `target/release/fragile-notepad.exe` on Windows or
`target/release/fragile-notepad` on Unix. Default features compile both renderers;
use `cargo build --release --locked --no-default-features` for a software-only
binary. Windows release builds use the GUI subsystem; `--help`, `--version`, and
CLI error reporting attach to the parent console when necessary.

Icons and syntax resources are embedded. Do not distribute `vendor/`, generated
profiling fixtures, or personal settings/session files with the binary. The nightly
workflow packages the executable in a Windows ZIP or Unix tarball; consult
`.github/workflows/nightly.yml` for current artifact names and target platforms.

Runtime settings and recovery snapshots use `settings.xml` and `session.json`
under the platform config directory. Compiled outline cache data uses the platform
cache directory. Both are resolved in [src/platform/paths.rs](src/platform/paths.rs),
including on macOS, which currently follows the Unix XDG/fallback paths. See
[Files and sessions](DEVELOPMENT.md#files-and-sessions) for exact locations,
command-line forwarding, recovery limits, and `--no-session` behavior.
