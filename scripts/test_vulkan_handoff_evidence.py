"""Check that handoff evidence proves the requested resize took effect."""

import csv
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "handoff", Path(__file__).with_name("check-vulkan-handoff.py"))
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


class ResizeEvidenceTests(unittest.TestCase):
    def test_resize_requires_presented_software_pixels_before_warmup_at_same_size(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.csv"
            def check(logical="700.0x440.0", physical="1050x660", result="ok", present_at=30):
                with path.open("w", newline="", encoding="utf-8") as stream:
                    writer = csv.writer(stream)
                    writer.writerow(("timestamp_us", "event", "elapsed_us", "detail"))
                    # Buffered output can arrive out of timestamp order.
                    writer.writerows([
                        (40, "backend_handoff_warm_start", 0, "width=1050 height=660"),
                        (present_at, "tiny_skia_present", 5, f"physical={physical} result={result}"),
                        (20, "winit_redraw_start", 0, f"logical={logical} physical={physical}"),
                        (10, "backend_handoff_prepare_start", 0, ""),
                    ])
                return handoff.resized_before_warmup(path)
            self.assertTrue(check())
            self.assertFalse(check(logical="640.0x380.0"))
            self.assertFalse(check(physical="700x440"))
            self.assertFalse(check(result="error"))
            self.assertFalse(check(present_at=50))
            self.assertFalse(check(present_at=15))


if __name__ == "__main__":
    unittest.main()
