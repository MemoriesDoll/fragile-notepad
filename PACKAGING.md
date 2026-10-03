# Packaging

Follow [build setup](README.md#build-from-source) and run the
[development checks](DEVELOPMENT.md#checks-and-previews), then:

```sh
cargo build --release --locked
```

Use `--no-default-features` for software-only builds. Output is
`target/release/fragile-notepad.exe` on Windows or `target/release/fragile-notepad`
on Unix. Windows release builds use the GUI subsystem and attach to the caller's
console for CLI output.

Icons and syntax resources are embedded. Asset generation exports
`target/app-icons/app.ico`, `app.icns`, and `app.png`; Windows embeds the ICO.
Distribute the project `LICENSE` and `assets/icons/NOTICE.txt` as
`ICON-NOTICES.txt` beside the binary. Preserve vendored and runtime license notices.

[The nightly workflow](.github/workflows/nightly.yml) defines archive names and
targets. Packages contain the executable and notices; settings, sessions,
profiling artifacts, and vendor source stay outside the package.

## macOS runtime

Install `molten-vk vulkan-loader vulkan-tools` with Homebrew, then:

```sh
mkdir -p dist
bash scripts/package-macos.sh dist/package
tar -C dist/package -czf dist/fragile-notepad-macos.tar.gz .
```

The package directory must be new. Distribute it whole: launcher,
`libexec/fragile-notepad`, Vulkan loader/MoltenVK dylibs, relative ICD manifest,
and licenses. The launcher configures discovery; Vulkan loads lazily.
This is a launcher package rather than an `.app` bundle.

Packaging rewrites library identities, rejects unresolved non-system dependencies,
and ad-hoc signs modified dylibs. Developer ID signing and notarization are separate.
Pinned source notices, checksums, and Homebrew metadata accompany the runtime;
notice collection requires network access.
