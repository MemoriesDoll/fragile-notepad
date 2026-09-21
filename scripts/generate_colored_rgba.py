"""Rasterize Fragile Notepad's original colored icons from editable SVG sources."""

from pathlib import Path

from rasterize_svg_icons import DEFAULT_SIZE, DEFAULT_SUPERSAMPLE, rasterize_svg

ROOT = Path(__file__).resolve().parents[1]
SVG_DIR = ROOT / "assets" / "icons" / "colored" / "svg"
RGBA_DIR = ROOT / "assets" / "icons" / "colored" / "rgba"


def main() -> None:
    RGBA_DIR.mkdir(parents=True, exist_ok=True)
    for source in sorted(SVG_DIR.glob("*.svg")):
        image = rasterize_svg(source, DEFAULT_SIZE, DEFAULT_SUPERSAMPLE, color=None)
        (RGBA_DIR / f"{source.stem}.rgba").write_bytes(image.tobytes())


if __name__ == "__main__":
    main()
