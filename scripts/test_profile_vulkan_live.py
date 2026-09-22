"""Regression checks for accepting/rejecting live presentation evidence."""

import csv
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "profile_vulkan_live", Path(__file__).with_name("profile-vulkan-live.py"))
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)


class LiveTraceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.trace = Path(self.directory.name) / "trace.csv"
        self.log = ("VULKAN_SUSTAIN_START timestamp_us=1000000\n"
                    "VULKAN_SUSTAIN_END timestamp_us=6000000\n")
        self.report = {
            "result": "ok",
            "strict_outcome": {"kind": "success", "windows": [{
                "window": "Id(1)", "backend": "Vulkan", "status": "presented",
                "renderer_family": "wgpu", "adapter": "Test adapter"}]},
            "trace_evidence": {"warm_backend": "Vulkan", "warm_submission_completed": True},
        }
        self.rows = []
        for index in range(60):
            timestamp = 3_100_000 + index * 41_667
            self.rows.extend([
                (timestamp, "winit_redraw_frame", 500,
                 "window=1 frame_delta_us=41667 interact_us=20 draw_us=30 present_us=400 status=ok"),
                (timestamp - 10, "fallback_present", 390, "backend=Vulkan status=ok"),
            ])

    def analyze(self):
        with self.trace.open("w", newline="", encoding="utf-8") as stream:
            writer = csv.writer(stream)
            writer.writerow(("timestamp_us", "event", "elapsed_us", "detail"))
            # Independent trace writers can append their headers midstream and
            # flush later timestamps before earlier ones.
            writer.writerow(("timestamp_us", "event", "elapsed_us", "detail"))
            writer.writerows(reversed(self.rows))
        return profile.analyze(self.trace, self.log, self.report, 2, 5)

    def test_buffered_trace_order_preserves_cadence_and_costs(self):
        summary = self.analyze()["windows"]["1"]
        self.assertEqual(summary["frames"], 60)
        self.assertAlmostEqual(summary["cadence_fps"], 24, places=3)
        self.assertEqual(summary["cpu_frame_us"]["median"], 500)
        self.assertEqual(summary["present_us"]["p95"], 400)

    def test_rejects_missing_or_failed_presentation(self):
        self.rows.pop()
        with self.assertRaisesRegex(ValueError, "unambiguous"):
            self.analyze()
        self.rows.append((3_100_000, "fallback_present", 390, "backend=Vulkan status=Lost"))
        with self.assertRaisesRegex(ValueError, "failed or non-Vulkan"):
            self.analyze()

    def test_rejects_software_fallback(self):
        self.rows[1] = (3_099_990, "fallback_present", 390, "backend=tiny-skia status=ok")
        with self.assertRaisesRegex(ValueError, "failed or non-Vulkan"):
            self.analyze()

    def test_rejects_missing_window_or_warm_up(self):
        self.report["strict_outcome"]["windows"].append({
            **self.report["strict_outcome"]["windows"][0], "window": "Id(2)"})
        with self.assertRaisesRegex(ValueError, "missing sustained frames"):
            self.analyze()
        self.report["trace_evidence"]["warm_submission_completed"] = False
        with self.assertRaisesRegex(ValueError, "warm-up/presentation"):
            self.analyze()

    def test_rejects_short_or_unbounded_capture(self):
        self.log = self.log.replace("6000000", "4000000")
        with self.assertRaisesRegex(ValueError, "prematurely"):
            self.analyze()
        self.log = ""
        with self.assertRaisesRegex(ValueError, "interval markers"):
            self.analyze()

    def test_rejects_animation_that_paused_partway_through_capture(self):
        self.rows = self.rows[:62]
        with self.assertRaisesRegex(ValueError, "do not cover"):
            self.analyze()


if __name__ == "__main__":
    unittest.main()
