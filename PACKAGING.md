# Packaging

Generate the embedded assets before building a release.

## Source Layout

- `src/` contains the application.
- `assets/` contains source assets and generated RGBA icon files consumed at
  compile time through `src/assets.rs`.
- `vendor/` contains customized dependencies; see [provenance and licenses](vendor/README.md).
- `scripts/` contains repeatable setup, asset generation, and CI entry points.

## Preparing a Checkout

Use the Rust stable toolchain and Python with
`python -m pip install -r scripts/requirements-assets.txt` (the same asset
tooling used by `.github/workflows/ci.yml`). Platform windowing dependencies must
also be available; the Linux CI job lists the required X11/Wayland packages.

## Generated Assets

RGBA icon and illustration files are generated from the tracked SVG sources.
Regenerate them on a fresh checkout and after changing any artwork source; all
generated `.rgba` files are ignored by Git:

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

The bunny sources live in `assets/illustrations/bunny/`. Standard asset generation
also exports `target/app-icons/app.ico`, `app.icns`, and `app.png` for packaging.
Large icons retain the rounded blue tile; the in-app title icon is transparent.
Windows builds embed the generated ICO in the executable through `build.rs`.
The macOS release uses a launcher and bundled Vulkan runtime, not an `.app`
bundle; `app.icns` is available for a future bundle's `CFBundleIconFile`.

Distribute the project `LICENSE` and `assets/icons/NOTICE.txt` (renamed to
`ICON-NOTICES.txt`) beside the binary. The nightly archive jobs include both;
manual packages must do the same to preserve the artwork copyright notices.
The original colored icons and illustrations are all rights reserved and excluded
from the project's BSD-3-Clause license; their full notices are included in
`ICON-NOTICES.txt`.

## Local Validation

The standard local validation entry point is:

```powershell
.\scripts\ci.ps1
```

On Linux or macOS:

```bash
bash scripts/ci.sh
```

The CI scripts run formatting, asset generation, Python asset checks, and the
following Cargo checks:

```powershell
cargo test
cargo test --locked -p iced_wgpu --lib
cargo test --locked -p cryoglyph --lib
cargo check --no-default-features
```

`cargo test` also compiles the application and examples. Additional renderer
diagnostics and environment requirements are in
[DEVELOPMENT.md](DEVELOPMENT.md). On Linux, `scripts/ci.sh` uses `xvfb-run` when
available. Both GitHub workflows install Mesa Lavapipe and select its Vulkan ICD
with `scripts/setup-ci-vulkan.sh`, which runs `vulkaninfo --summary` before
compilation. Linux GUI tests run under Xvfb, including the nightly release gates.
This exercises wgpu on a software Vulkan device; it does not validate physical
GPU drivers. The icon parity test requires an available wgpu adapter in CI unless
`FRAGILE_ALLOW_WGPU_PARITY_SKIP=1` explicitly opts out; skipped GPU checks are not
GPU-validation evidence.

Hardware rendering now compiles only Vulkan, including Vulkan portability on
macOS. Windows CI selects the Vulkan loader and SwiftShader ICD shipped with
the runner's Chrome installation via `scripts/setup-ci-vulkan.ps1`. This is also
software Vulkan, and is used only for CI. Windows distribution continues to use
the user's graphics-driver Vulkan runtime.

For local macOS validation, install `molten-vk`, `vulkan-loader`, and
`vulkan-tools` with Homebrew, then run in the same shell:

```bash
source scripts/setup-macos-vulkan.sh
vulkaninfo --summary
source scripts/ci.sh
```

Source the CI script here: launching another system shell can cause macOS SIP
to remove `DYLD_LIBRARY_PATH`.

## Release Build

After asset generation and validation:

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

On macOS, after the release build and Homebrew runtime installation:

```bash
mkdir -p dist
bash scripts/package-macos.sh dist/package
tar -C dist/package -czf dist/fragile-notepad-macos.tar.gz .
```

The package directory must be new. Distribute the entire directory: the
`fragile-notepad` launcher, `libexec/fragile-notepad`, the loader and MoltenVK
dylibs in `lib/`, the relative ICD manifest in `share/vulkan/icd.d/`, and
licenses. The launcher configures discovery and then executes the application;
Vulkan still loads lazily after software startup. Packaging rewrites library
identities, rejects unresolved non-system dependencies, and ad-hoc signs the
modified dylibs. This does not provide Developer ID signing or notarization.
Pinned source notices (including MoltenVK's static dependencies), source
checksums, runtime checksums, and Homebrew build metadata accompany the package.
Collecting these notices requires network access to the pinned upstream sources.
Nightly gates test handoff against the packaged runtime before creating the
archive. Native macOS packaging and launch results remain unverified locally.

Runtime settings and recovery snapshots use `settings.xml` and `session.json`
under the platform config directory. Compiled outline cache data uses the platform
cache directory. Both are resolved in [src/platform/paths.rs](src/platform/paths.rs),
including on macOS, which currently follows the Unix XDG/fallback paths. See
[Files and sessions](DEVELOPMENT.md#files-and-sessions) for exact locations,
command-line forwarding, recovery limits, and `--no-session` behavior.
