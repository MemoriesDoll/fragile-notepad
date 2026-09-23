# Fragile Notepad icon artwork

All 53 maintained icons have editable SVG sources. The app embeds 47 of them;
six additional vectors are available for future use. Project artwork lives in
`colored/` and is exposed through the `ui::icons::colored` module.

## Design

The colored set preserves the editor's familiar document/folder/disk vocabulary
with flat fills, restrained color, rounded outlines, and simpler silhouettes.
Artwork uses a 22-unit square canvas, usually 1.5-unit colored outlines and
1.8-unit control strokes, with optical padding. Monochrome controls and shortcuts
inherit the theme text color, including the tab pins. Related states share their
base geometry, but color is never their only distinguishing feature.
Context-menu submenu and scroll arrows also use the shared chevron artwork
instead of font-dependent symbols.

| Action | Distinction |
| --- | --- |
| Save / Save All / Save As | Single disk / stacked disks / disk and pencil |
| Close / Close All / Delete | Crossed document / stacked crossed documents / bin |
| Find / Replace | Magnifying glass / A and B with exchange arrows |
| Zoom in / out | Clear plus / minus inside the same lens |
| Word wrap / indent guides / all characters | Bent return arrow / dashed vertical guide / pilcrow |
| Function list | Braces surrounding three rows |
| Saved / unsaved | Document with check / document with pencil |
| Read-only / system read-only / monitoring | Lock / shield / eye |
| Pinned / unpinned | Solid / outlined pin with matching geometry |

The native title-bar maximize/restore squares and editor fold/whitespace geometry
remain native renderer primitives. They were reviewed alongside the asset set;
their simple geometry needs no replacement image. The About quill is an
illustration, with its own provenance under `assets/illustrations`.

## Sources and licenses

Online references consulted on 2026-09-22:

- [Lucide design principles](https://lucide.dev/contribute/icons/design-principles)
  ([source](https://github.com/lucide-icons/lucide/blob/main/docs/contribute/icons/design-principles.md)):
  optical balance, safe zones, simple curves, spacing, and low visual density.
  Used as design guidance only; no Lucide SVG paths or illustrations were copied.
- [Heroicons](https://github.com/tailwindlabs/heroicons): small-size icon families,
  rounded strokes, and `currentColor` usage. The existing local control family
  was refined; its [MIT notice](heroicons/LICENSE) is retained.
- [Bootstrap Icons](https://github.com/twbs/icons): existing shortcut and pin
  family. These have been simplified and normalized locally; the Windows key
  mark retains the upstream proportions with added padding. The verified
  [MIT notice](bootstrap/LICENSE) is retained. The Windows symbol identifies a
  keyboard key; the icon copyright license does not grant trademark rights.

`colored/` is original project artwork: **Copyright (c) 2026, Fragile Notepad
authors. All rights reserved.** Its SVG sources, generated raster assets, and
reproductions are explicitly excluded from the project's BSD-3-Clause license.
See [the artwork notice](colored/LICENSE) for the terms. No downloaded replacement
icon pack or traced proprietary artwork is used.

[NOTICE.txt](NOTICE.txt) contains the icon notices for binary redistribution.
Release archives include it as `ICON-NOTICES.txt` alongside the project license.

## Regeneration and review

Run `scripts/generate_icon_assets.ps1` (Windows) or
`bash scripts/generate_icon_assets.sh` (Unix) from the repository root. Python
with Pillow handles this UI icon set; install the complete asset tooling from
`scripts/requirements-assets.txt` (the bunny illustration also needs resvg).
Each UI vector is rasterized at 8x and
downsampled once to straight-alpha 22x22 RGBA, using the same bytes for CPU/GPU
renderers. Generated RGBA is ignored by Git.

The rasterizer intentionally supports a small SVG subset: paths, flat six-digit
hex fills/strokes or `currentColor`, and rounded caps/joins. It does not support
transforms, groups with inherited paint, gradients, or arbitrary SVG markup.
Filled multi-subpath artwork uses even-odd filling; declare `fill-rule="evenodd"`
if adding a shape with holes. Keep new sources within this subset.

`python -m unittest discover -s scripts -p test_icon_assets.py` checks paint,
mask geometry, and clipping. `cargo test --test render_icon_parity` covers all
embedded assets at 100%, 150%, and 200% display scale with CPU/GPU renderers.
