# Icons

Editable SVGs live in `colored/`, `heroicons/`, and `bootstrap/`.
Colored artwork uses a 22-unit canvas, flat fills, and rounded outlines;
monochrome controls inherit theme text color.

## Licenses

[Colored artwork](colored/LICENSE) is original project work, all rights reserved,
and excluded from the code's BSD license. Modified [Heroicons](heroicons/LICENSE)
and [Bootstrap Icons](bootstrap/LICENSE) retain MIT notices.
Redistribute [NOTICE.txt](NOTICE.txt) as `ICON-NOTICES.txt`.

## Generation

Run the [standard asset scripts](../../README.md#build-from-source).
UI vectors rasterize at 8x and downsample to straight-alpha 22×22 RGBA shared by
both renderers. Generated files are ignored by Git.

The rasterizer supports paths, flat six-digit hex or `currentColor` paint,
and rounded caps/joins. Transforms, inherited group paint, and gradients are
unsupported. Use `fill-rule="evenodd"` for filled shapes with holes.
