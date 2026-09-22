"""Run strict CPU-to-Vulkan handoff scenarios with the real About animation."""

import argparse
import csv
import json
import os
from pathlib import Path
import subprocess
import tempfile


def resized_before_warmup(trace_path):
    """Require an actual resized software frame and matching Vulkan warm size."""
    with trace_path.open(encoding="utf-8", newline="") as stream:
        events = [row for row in csv.DictReader(stream)
                  if row["timestamp_us"].isdigit()]
    def fields(row):
        return dict(field.split("=", 1) for field in row["detail"].split() if "=" in field)
    prepare = next((int(row["timestamp_us"]) for row in events
                    if row["event"] == "backend_handoff_prepare_start"), None)
    warm = next((row for row in events if row["event"] == "backend_handoff_warm_start"), None)
    if prepare is None or warm is None:
        return False
    warm_at = int(warm["timestamp_us"])
    size = fields(warm)
    physical = f"{size.get('width')}x{size.get('height')}"
    starts = [int(row["timestamp_us"]) for row in events
              if row["event"] == "winit_redraw_start"
              and prepare <= int(row["timestamp_us"]) < warm_at
              and fields(row).get("logical") == "700.0x440.0"
              and fields(row).get("physical") == physical]
    # This scenario has one live window. Check the software presentation itself,
    # not just the delivery of a resize event or an attempted redraw.
    return bool(starts) and any(
        row["event"] == "tiny_skia_present"
        and min(starts) < int(row["timestamp_us"]) < warm_at
        and fields(row).get("physical") == physical
        and fields(row).get("result") == "ok"
        for row in events)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/vulkan-handoff"))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="run-", dir=args.output)).resolve()
    cases = [(scenario, "none") for scenario in (
        "single-window", "multi-window", "resize-during-preparing",
        "close-during-preparing", "close-during-commit-pending",
    )] + [("single-window", failure) for failure in (
        "prepare", "warm", "commit", "first-present",
    )]
    # Each child configures its own injection hooks; never inherit stale ones.
    env = {key: value for key, value in os.environ.items() if not (
        key.startswith("FRAGILE_BACKEND_SWITCH_PROBE_")
        or key.startswith("FRAGILE_NOTEPAD_RENDER_")
    )}
    env.update(WGPU_BACKEND="vulkan", FRAGILE_PERF_TRACE="1")
    failed = []
    for scenario, failure in cases:
        name = f"{scenario}-{failure}"
        directory = output / name
        directory.mkdir()
        child_env = dict(env, FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR=str(directory),
                         FRAGILE_PERF_TRACE_DIR=str(directory))
        try:
            with (directory / "probe.log").open("w", encoding="utf-8") as log:
                result = subprocess.run(
                    [str(binary), f"--scenario={scenario}", f"--fail={failure}"],
                    env=child_env, stdout=log, stderr=subprocess.STDOUT,
                    timeout=60, check=False,
                    creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
                )
            reports = list(directory.glob("*.json"))
            report = json.loads(reports[0].read_text(encoding="utf-8")) if len(reports) == 1 else {}
            passed = (result.returncode == 0 and report.get("result") == "ok"
                      and report.get("scenario") == scenario
                      and report.get("failure") == failure)
            if failure == "none" and scenario != "close-during-preparing":
                outcome = report.get("strict_outcome") or {}
                windows = outcome.get("windows", [])
                evidence = report.get("trace_evidence", {})
                passed = (passed and outcome.get("kind") == "success" and bool(windows)
                          and all(window.get("backend") == "Vulkan"
                                  and window.get("renderer_family") == "wgpu"
                                  and window.get("status") == "presented" for window in windows)
                          and evidence.get("warm_backend") == "Vulkan"
                          and evidence.get("warm_submission_completed") is True)
            if scenario == "resize-during-preparing":
                passed = (passed and report.get("requested_resize_observed") is True
                          and resized_before_warmup(directory / "fragile-perf.csv"))
            reason = report.get("reason", "missing or ambiguous result report")
            if not passed and report.get("result") == "ok":
                reason = "result reported ok but required Vulkan presentation/resize evidence was missing"
        except (subprocess.TimeoutExpired, OSError, ValueError) as error:
            passed, reason = False, str(error)
        print(f"{'PASS' if passed else 'FAIL'} {name}: {reason}", flush=True)
        if not passed:
            failed.append(name)
    print(f"Vulkan handoff evidence: {output}", flush=True)
    if failed:
        raise SystemExit(f"Failed scenarios: {', '.join(failed)}")


if __name__ == "__main__":
    main()
