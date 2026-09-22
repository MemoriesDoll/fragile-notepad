# Bunny application artwork

The supplied BunnyNotebook SVG final revision (2026-09-21) is the source of
the application identity. Like the other artwork in this directory, these
vectors and their derivatives are covered by [../LICENSE](../LICENSE), not
the code's BSD license.

- `app.svg`: unchanged `01_Bunny_Notebook_Master.svg`, including the rounded
  blue background. Used in the README and native window icon; About derives
  separate background and foreground layers from this master.
- `title-bar.svg`: `04_Bunny_Notebook_Small.svg` with only the
  `small-background` layer removed. Drawn at 24 logical pixels in both title-bar
  styles; no blue tile behind the rabbit. The 64px raster supports high DPI.

Install `python -m pip install -r scripts/requirements-assets.txt`, then run
`python scripts/generate_app_icons.py` (also called by both standard asset
generation scripts). The build-only resvg dependency handles the source
gradients and transforms; Pillow downsamples in premultiplied-alpha space.
The application still uses embedded straight RGBA and no runtime SVG decoder.

Generated `app.rgba` (256px) and `title-bar.rgba` (64px) are ignored by Git.
Packaging exports are written to `target/app-icons/app.ico`, `app.icns`, and
`app.png`. The ICO includes 16, 24, 32, 48, 64, 128, and 256px sizes. The ICNS
and PNG retain the rounded background for large-icon use.

About reserves an 80px slot, with a 64px background and a 96px foreground canvas.
The bunny extends slightly beyond the tile and floats by 2.5px in a four-second
cycle, with a subtle sideways drift and a quieter 0.5px background motion.
The left loose paper is a separate `about-paper.rgba` layer behind the bunny,
with a 5.5-second cycle, 3.5px vertical float and 1.5px sideways drift. The right
paper stays with the bunny. All layers use the same pausable animation clock.
Soft blue, lavender and rose wisps flow toward the quill, tapering into its nib;
the trail keeps the title area clear and fades out at its boundaries.
The shared 24fps clock keeps deadlines anchored across late display frames,
and retains fade and focus/visibility pausing. The 384px `about-background`
and `about-bunny` rasters are generated from the master layers. Once per four
seconds, the eyes close and reopen over 250ms using generated half-open and
closed foreground frames (`about-bunny-half.rgba`, `about-bunny-closed.rgba`).
Only the two eye groups change; the original SVG and static icons stay intact.
The Windows title places the rabbit before the caption; the macOS
style places it opposite the traffic lights in the existing 84px side slot
so the caption stays centered.

Run `cargo run --example preview_branding` to render the actual Iced About and
title-bar widgets in both styles and themes to `target/bunny-review/`. The
Windows/light series records one four-second floating cycle. This visual
preview does not emulate macOS window-manager behavior.
Add `-- --vulkan` to render through Vulkan into `target/bunny-review-vulkan/`.
In the application, opening About requests Vulkan through the shared renderer
handoff when hardware acceleration is enabled. Software-only mode and failure
fallback remain supported; the animation clock stays at 24fps on either backend.

## Visual review checklist

- About replaces the FN placeholder with the background-backed bunny.
- The floating motion, independent left paper, slight overflow, and brief blink
  are visible at actual size.
- Both eyes close together; fur, notebook and background keep their geometry.
- Closing About or unfocusing pauses the shared clock.
- Title icons remain transparent and legible at 24px in light and dark themes.
- Windows keeps a clear gap between the icon and caption; macOS keeps its
  traffic lights and centered caption. Inactive icons dim with the title bar.
- Long titles leave window controls accessible; dragging and double-clicking
  continue to work with either style.
- README and application icons retain the rounded blue background.
- Native macOS window-manager behavior requires a separate check on a Mac.
