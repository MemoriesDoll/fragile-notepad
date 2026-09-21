<img src="assets/illustrations/macaw-quill.svg" align="right" width="72" alt="">

# Fragile Notepad

A desktop text editor for notes and source files. Written in Rust with
[Iced](https://iced.rs), for Windows, Linux, and macOS.

[Releases](https://github.com/MemoriesDoll/fragile-notepad/releases)
&nbsp;·&nbsp; [Development](DEVELOPMENT.md)
&nbsp;·&nbsp; [Architecture](ARCHITECTURE.md)

## Build from source

You need Git, a current stable [Rust toolchain](https://rustup.rs), and Python
3.10 or newer with [Pillow](https://pillow.readthedocs.io/en/stable/installation/basic-installation.html)
installed in your Python environment. Use the native build tools for your platform.
The Linux windowing libraries are listed in the [CI workflow](.github/workflows/ci.yml).

```sh
git clone https://github.com/MemoriesDoll/fragile-notepad.git
cd fragile-notepad
```

Run both setup steps before Cargo: dependencies need local patches, and raster
assets are generated rather than stored in Git.

**Windows — PowerShell**

```powershell
.\scripts\setup-vendor.ps1 apply
.\scripts\generate_icon_assets.ps1
cargo run --release --locked
```

**Linux / macOS**

```sh
bash scripts/setup-vendor.sh apply
bash scripts/generate_icon_assets.sh
cargo run --release --locked
```

Add `--no-default-features` to the Cargo command for a software-only build.
See [Packaging](PACKAGING.md) for release builds.

For local checks, run `.\scripts\ci.ps1` on Windows or `bash scripts/ci.sh`
on Linux and macOS.

## License

The project code is licensed under [BSD-3-Clause](LICENSE).
The original artwork in [`assets/icons/colored/`](assets/icons/colored/LICENSE)
and [`assets/illustrations/`](assets/illustrations/LICENSE),
including generated rasters and reproductions, is **all rights reserved** and
excluded from that license. Other bundled icons retain their MIT licenses;
see [Artwork notices](assets/icons/NOTICE.txt).
