"""Focused checks for the path-only icon renderer. Run with unittest discover."""

from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from rasterize_svg_icons import DEFAULT_COLOR, rasterize_svg

ROOT = Path(__file__).resolve().parents[1]


class IconAssetsTest(unittest.TestCase):
    def render(self, paths, color=None):
        with TemporaryDirectory() as directory:
            source = Path(directory) / "icon.svg"
            source.write_text(
                '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 22 22">'
                + paths + '</svg>', encoding="utf-8"
            )
            return rasterize_svg(source, 22, 8, color)

    def test_fill_and_stroke_keep_separate_colors(self):
        image = self.render(
            '<path d="M4 4 H18 V18 H4 Z" fill="#f5f8fc" '
            'stroke="#397bb6" stroke-width="2"/>'
        )
        self.assertEqual(image.getpixel((11, 11)), (245, 248, 252, 255))
        edge = image.getpixel((4, 11))
        self.assertLess(edge[0], 75)
        self.assertGreater(edge[2], 165)
        self.assertEqual(image.getpixel((0, 0))[3], 0)

    def test_monochrome_override_preserves_geometry(self):
        paths = (
            '<path d="M4 4 H18 V18 H4 Z" fill="#f5f8fc" '
            'stroke="#397bb6" stroke-width="2"/>'
            '<path d="M7 11 H15" fill="none" '
            'stroke="currentColor" stroke-width="2"/>'
        )
        source = self.render(paths)
        mask = self.render(paths, DEFAULT_COLOR)
        self.assertEqual(source.getchannel("A").tobytes(), mask.getchannel("A").tobytes())
        self.assertEqual(mask.getpixel((11, 11)), DEFAULT_COLOR)
        self.assertEqual(mask.getpixel((11, 7)), DEFAULT_COLOR)

    def test_complete_icon_sources_have_visible_unclipped_artwork(self):
        for family in (ROOT / "assets/icons").iterdir():
            if not family.is_dir():
                continue
            for source in (family / "svg").glob("*.svg"):
                with self.subTest(icon=source.name, family=family.name):
                    image = rasterize_svg(source, 22, 8, None)
                    alpha = image.getchannel("A")
                    self.assertGreater(sum(a > 32 for a in alpha.tobytes()), 10)
                    border = [alpha.getpixel((i, j)) for k in range(22)
                              for i, j in ((0, k), (21, k), (k, 0), (k, 21))]
                    self.assertLessEqual(max(border), 32, "visible artwork touches canvas edge")


if __name__ == "__main__":
    unittest.main()
