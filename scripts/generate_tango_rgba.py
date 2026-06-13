import functools
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
PNG_DIR = ROOT / "assets" / "icons" / "tango" / "png"
RGBA_DIR = ROOT / "assets" / "icons" / "tango" / "rgba"
SIZE = (22, 22)

BASE_ICON_NAMES = [
    "document-new", "document-open", "document-save", "document-save-as",
    "document-print", "edit-cut", "edit-copy", "edit-paste", "edit-undo",
    "edit-redo", "edit-find", "edit-find-replace", "format-justify-fill",
    "format-indent-more", "process-stop", "edit-delete", "text-x-generic",
    "text-x-generic-template", "text-x-script", "accessories-character-map",
    "emblem-important", "emblem-favorite"
]

WHITE = (255, 255, 255, 255)

@functools.lru_cache(maxsize=None)
def _load_base_png(filename: str) -> Image.Image:
    """Loads, converts, and scales the base image asset once."""
    with Image.open(PNG_DIR / filename) as img:
        image = img.convert("RGBA")
    if image.size != SIZE:
        image = image.resize(SIZE, Image.Resampling.LANCZOS)
    return image


def load_icon(filename: str) -> Image.Image:
    """Returns a fresh copy of a base icon asset from cache."""
    return _load_base_png(filename).copy()


def write_rgba(name: str, image: Image.Image) -> None:
    """Writes raw RGBA image bytes to the target directory."""
    (RGBA_DIR / f"{name}.rgba").write_bytes(image.tobytes())


def generate_tab_document_icon(name: str, badge: str | None) -> None:
    image = load_icon("text-x-generic.png")
    if badge is None:
        write_rgba(name, image)
        return

    draw = ImageDraw.Draw(image)

    if badge == "unsaved":
        draw.ellipse((11, 11, 21, 21), fill=(136, 38, 28, 210))
        draw.ellipse((12, 12, 20, 20), fill=(232, 84, 43, 255))
        draw.rectangle((15, 14, 16, 18), fill=WHITE)
        draw.line((15, 19, 16, 19), fill=WHITE)  # Cleaned up from distinct points
    elif badge == "read-only":
        draw.rectangle((11, 13, 20, 20), fill=(58, 72, 88, 230))
        draw.rectangle((12, 14, 19, 19), fill=(98, 116, 136, 255))
        draw.arc((13, 9, 18, 16), 180, 360, fill=(58, 72, 88, 255), width=2)
        draw.rectangle((15, 16, 16, 18), fill=(238, 243, 246, 255))
    elif badge == "system-read-only":
        draw.polygon(
            [(16, 10), (21, 12), (20, 17), (16, 21), (12, 17), (11, 12)],
            fill=(66, 54, 103, 220),
        )
        draw.polygon(
            [(16, 11), (20, 13), (19, 17), (16, 20), (13, 17), (12, 13)],
            fill=(119, 92, 181, 255),
        )
        draw.line((14, 16, 18, 16), fill=WHITE, width=2)
    elif badge == "monitoring":
        draw.ellipse((10, 12, 21, 19), fill=(37, 98, 72, 230))
        draw.ellipse((11, 13, 20, 18), fill=(74, 155, 105, 255))
        draw.ellipse((14, 13, 18, 17), fill=(236, 255, 241, 255))
        draw.ellipse((15, 14, 17, 16), fill=(37, 98, 72, 255))
    else:
        raise ValueError(f"Unknown tab document badge: {badge}")

    write_rgba(name, image)


def generate_zoom_icon(name: str, sign: str) -> None:
    image = load_icon("edit-find.png")
    draw = ImageDraw.Draw(image)

    # Main dark container backing for the magnifying glass center sign symbol
    draw.rectangle((4, 9, 12, 12), fill=(40, 70, 95, 230))

    if sign == "+":
        draw.rectangle((7, 6, 9, 15), fill=(40, 70, 95, 230))

    # Shared Horizontal Line Component
    draw.rectangle((5, 10, 11, 11), fill=WHITE)

    if sign == "+":
        # Additional Vertical Line Component for Plus
        draw.rectangle((8, 7, 8, 14), fill=WHITE)

    write_rgba(name, image)


def generate_tab_close_icon() -> None:
    image = Image.new("RGBA", SIZE, (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    # Base button container layout matching style guide dimensions
    draw.rectangle((5, 5, 16, 16), fill=(114, 48, 78, 255))
    draw.rectangle((6, 6, 15, 15), fill=(177, 73, 113, 255))
    draw.rectangle((7, 7, 14, 14), fill=(197, 84, 125, 255))

    # Clean, pixel-perfect close cross generation
    for i in range(5):
        draw.rectangle((8 + i, 8 + i, 9 + i, 9 + i), fill=WHITE)
        draw.rectangle((12 - i, 8 + i, 13 - i, 9 + i), fill=WHITE)

    write_rgba("tab-close", image)


# --- Main Orchestration Loop ---
def main() -> None:
    RGBA_DIR.mkdir(parents=True, exist_ok=True)

    # Generate standard fallback assets
    for name in BASE_ICON_NAMES:
        write_rgba(name, load_icon(f"{name}.png"))

    # Generate modified/constructed dynamic variants
    generate_tab_document_icon("tab-document-saved", None)
    generate_tab_document_icon("tab-document-unsaved", "unsaved")
    generate_tab_document_icon("tab-document-read-only", "read-only")
    generate_tab_document_icon("tab-document-system-read-only", "system-read-only")
    generate_tab_document_icon("tab-document-monitoring", "monitoring")
    
    generate_zoom_icon("zoom-in", "+")
    generate_zoom_icon("zoom-out", "-")
    
    generate_tab_close_icon()


if __name__ == "__main__":
    main()
