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

```powershell
.\scripts\setup-vendor.ps1 apply
```

On Linux or macOS:

```bash
bash scripts/setup-vendor.sh apply
```

## Generated Assets

RGBA icon files are generated from the tracked SVG/PNG sources. Regenerate them
after changing any icon source:

```powershell
.\scripts\generate_icon_assets.ps1
```

On Linux or macOS:

```bash
bash scripts/generate_icon_assets.sh
```

The application embeds generated icons through `src/assets.rs`. New embedded
assets should be registered there instead of using `include_bytes!` from UI
modules.

## Local Validation

The standard local validation entry point is:

```powershell
.\scripts\ci.ps1
```

On Linux or macOS:

```bash
bash scripts/ci.sh
```

For changes touching rendering, platform paths, or vendored patches, also run:

```powershell
cargo check
cargo test
cargo check --examples
cargo check --no-default-features
```

## Release Build

After vendor setup, asset generation, and validation:

```powershell
cargo build --release
```

The Windows release binary uses the GUI subsystem via `src/main.rs`. Runtime
settings are stored under the platform config directory, and compiled outline
cache data is stored under the platform cache directory; both paths are defined
in `src/platform.rs`.
