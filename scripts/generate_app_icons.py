"""Rasterize the supplied bunny vectors with gradients and transforms intact.

resvg is build tooling only; the application embeds straight RGBA pixels.
"""
from io import BytesIO
from pathlib import Path
from xml.etree import ElementTree as ET

from PIL import Image
import resvg_py

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "assets/illustrations/bunny"


def render(name, size, blink=None):
    source = (ART / f"{name}.svg").read_text(encoding="utf-8")
    if blink is not None:
        root = ET.fromstring(source)
        face = root.find(".//{*}g[@id='face']")
        if face is None or len(face) != 2:
            raise ValueError("Bunny artwork must contain the two named face eye groups")
        for eye, (x, y, radius) in zip(face, [(437, 400, 19), (583, 373, 18)]):
            if blink == "closed":
                for child in list(eye):
                    eye.remove(child)
                ET.SubElement(eye, "{http://www.w3.org/2000/svg}path", {
                    "d": f"M{x-radius} {y} Q{x} {y+10} {x+radius} {y}",
                    "fill": "none", "stroke": "#203954",
                    "stroke-width": "7", "stroke-linecap": "round",
                })
            else:
                # Scale the eye and its highlight together in its rotated local
                # coordinates. The head, fur and background are never squashed.
                eye.set("transform", eye.get("transform", "") +
                        f" translate(0 {y}) scale(1 .4) translate(0 {-y})")
        source = ET.tostring(root, encoding="unicode")
    png = resvg_py.svg_to_bytes(
        svg_string=source,
        width=size * 4,
        height=size * 4,
    )
    return Image.open(BytesIO(png)).convert("RGBA").resize(
        (size, size), Image.Resampling.LANCZOS
    )


def generate():
    app = render("app", 256)
    (ART / "app.rgba").write_bytes(app.tobytes())
    for blink in ("half", "closed"):
        (ART / f"app-{blink}.rgba").write_bytes(render("app", 256, blink).tobytes())
    (ART / "title-bar.rgba").write_bytes(render("title-bar", 64).tobytes())
    # Package assets are generated alongside the embedded rasters, never tracked.
    output = ROOT / "target/app-icons"
    output.mkdir(parents=True, exist_ok=True)
    app.save(output / "app.ico", sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])
    large = render("app", 1024)
    large.save(output / "app.png")
    large.save(output / "app.icns")


if __name__ == "__main__":
    generate()
