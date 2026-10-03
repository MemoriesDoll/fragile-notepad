# Bunny artwork

`app.svg` is the supplied BunnyNotebook master with its rounded blue background.
`title-bar.svg` is the small variant with its background removed, displayed at
24 logical pixels. Sources and derivatives use [the artwork license](../LICENSE).

`python scripts/generate_app_icons.py` (included in standard asset generation)
uses resvg and Pillow to produce embedded straight RGBA and
`target/app-icons/app.ico`, `app.icns`, and `app.png`.
The ICO includes 16–256px sizes. Generated files are ignored by Git.

About separates background, bunny, and paper layers, with eye-only blink variants
and a shared pausable animation clock. Both renderers preserve fractional motion.
See [branding previews](../../../DEVELOPMENT.md#checks-and-previews).
