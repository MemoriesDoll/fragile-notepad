"""Measure traced CPU frame costs and cadence after a strict Vulkan handoff.

These are instrumented wall times, not GPU timings or display scanout latency.
About retains its own 60 Hz scheduler; editor scrolling advances three rows per
24 Hz timer tick. No changing probe labels or frame subscription drive redraws.
"""

import argparse
from bisect import bisect_left, bisect_right
import csv
import json
import math
import os
from pathlib import Path
import re
import statistics
import subprocess
import tempfile


def fields(detail):
    return dict(token.split("=", 1) for token in detail.split() if "=" in token)


def distribution(values):
    ordered = sorted(values)
    return {"median": statistics.median(ordered),
            "p95": ordered[math.ceil(len(ordered) * .95) - 1],
            "max": ordered[-1]}


def analyze(trace_path, log, report, warmup_seconds, seconds):
    starts = re.findall(r"VULKAN_SUSTAIN_START timestamp_us=(\d+)", log)
    ends = re.findall(r"VULKAN_SUSTAIN_END timestamp_us=(\d+)", log)
    if len(starts) != 1 or len(ends) != 1:
        raise ValueError("missing or ambiguous sustained interval markers")
    begin, end = int(starts[0]), int(ends[0])
    if end - begin < seconds * 1_000_000:
        raise ValueError("sustained interval ended prematurely")
    cutoff = begin + warmup_seconds * 1_000_000
    outcome = report.get("strict_outcome") or {}
    evidence = report.get("trace_evidence") or {}
    windows = outcome.get("windows", [])
    if (report.get("result") != "ok" or outcome.get("kind") != "success"
            or evidence.get("warm_backend") != "Vulkan"
            or evidence.get("warm_submission_completed") is not True
            or not windows or any(window.get("backend") != "Vulkan"
                                  or window.get("status") != "presented"
                                  or window.get("renderer_family") != "wgpu"
                                  for window in windows)):
        raise ValueError("strict Vulkan warm-up/presentation evidence missing")
    with trace_path.open(encoding="utf-8") as stream:
        rows = [row for row in csv.DictReader(stream)
                if row["timestamp_us"].isdigit()]
    # Writers buffer independently; physical CSV order is not timestamp order.
    presents = sorted(((int(row["timestamp_us"]), fields(row["detail"]))
                       for row in rows if row["event"] == "fallback_present"),
                      key=lambda event: event[0])
    present_times = [timestamp for timestamp, _ in presents]
    for timestamp, detail in presents:
        if cutoff <= timestamp <= end and (detail.get("backend") != "Vulkan"
                                           or detail.get("status") != "ok"):
            raise ValueError("failed or non-Vulkan presentation during measurement")
    frames = {}
    for row in rows:
        if row["event"] != "winit_redraw_frame":
            continue
        timestamp = int(row["timestamp_us"])
        elapsed = int(row["elapsed_us"])
        if timestamp - elapsed < cutoff or timestamp > end:
            continue
        detail = fields(row["detail"])
        first = bisect_left(present_times, timestamp - elapsed)
        last = bisect_right(present_times, timestamp)
        if last - first != 1 or detail.get("status") != "ok":
            raise ValueError("frame lacks an unambiguous successful presentation")
        frames.setdefault(detail["window"], []).append((timestamp, elapsed, detail))
    expected = {re.fullmatch(r"Id\((\d+)\)", window["window"])[1] for window in windows}
    if set(frames) != expected:
        raise ValueError("missing sustained frames for a live window (possibly unfocused)")
    summaries = {}
    for window, samples in frames.items():
        samples.sort(key=lambda sample: sample[0])
        if len(samples) < 30:
            raise ValueError(f"window {window} has only {len(samples)} frames; may be paused/occluded")
        if (samples[-1][0] - samples[0][0] < (end - cutoff) * .8
                or samples[0][0] - cutoff > 1_000_000
                or end - samples[-1][0] > 1_000_000):
            raise ValueError(f"window {window} frames do not cover the measurement interval")
        intervals = [int(sample[2]["frame_delta_us"]) for sample in samples[1:]]
        summaries[window] = {
            "frames": len(samples),
            "cadence_fps": (len(samples) - 1) * 1_000_000 / sum(intervals),
            "frame_interval_us": distribution(intervals),
            "intervals_over_62_5_ms": sum(value > 62_500 for value in intervals),
            "cpu_frame_us": distribution([sample[1] for sample in samples]),
            **{key: distribution([int(sample[2][key]) for sample in samples])
               for key in ("interact_us", "draw_us", "present_us")},
        }
        geometry = [fields(row["detail"]) for row in rows
                    if row["event"] == "winit_redraw_start"
                    and cutoff <= int(row["timestamp_us"]) <= end
                    and fields(row["detail"]).get("window") == window]
        summaries[window]["physical_sizes"] = sorted({item["physical"] for item in geometry})
        summaries[window]["scales"] = sorted({float(item["scale"]) for item in geometry})
    return {"warmup_seconds": warmup_seconds, "requested_seconds": seconds,
            "observed_seconds": (end - begin) / 1_000_000,
            "adapters": sorted({window["adapter"] for window in windows}),
            "windows": summaries,
            "measurement": "traced redraw wall times (interaction/draw/present, excluding separate UI rebuilding) and cadence; not GPU time or scanout latency"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/vulkan-live"))
    parser.add_argument("--seconds", type=int, default=12, choices=range(5, 301), metavar="5..300")
    parser.add_argument("--warmup-seconds", type=float, default=2)
    parser.add_argument("--workload", choices=("about", "editor", "plain-text"), default="about")
    parser.add_argument("--windows", type=int, choices=(1, 2), default=1)
    args = parser.parse_args()
    if not 0 <= args.warmup_seconds <= args.seconds - 2:
        parser.error("warm-up must leave at least two seconds of measurement")
    if args.windows == 2 and args.workload == "about":
        parser.error("About pauses on focus loss; use one window for sustained animation")
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix=f"{args.workload}-", dir=args.output)).resolve()
    env = {key: value for key, value in os.environ.items() if not (
        key.startswith("FRAGILE_BACKEND_SWITCH_PROBE_")
        or key.startswith("FRAGILE_NOTEPAD_RENDER_"))}
    env.update(WGPU_BACKEND="vulkan", FRAGILE_PERF_TRACE="1",
               FRAGILE_BACKEND_SWITCH_PROBE_RESULT_DIR=str(output),
               FRAGILE_PERF_TRACE_DIR=str(output))
    command = [str(binary), f"--sustain-seconds={args.seconds}",
               "--scenario=single-window" if args.windows == 1 else "--scenario=multi-window"]
    if args.workload != "about":
        command.append(f"--{args.workload}")
    print(f"Live Vulkan evidence: {output}", flush=True)
    with (output / "probe.log").open("w", encoding="utf-8") as log:
        process = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT,
                                 timeout=args.seconds + 60, check=False,
                                 creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    if process.returncode:
        raise SystemExit(f"probe exited {process.returncode}; see {output / 'probe.log'}")
    reports = list(output.glob("*.json"))
    if len(reports) != 1:
        raise SystemExit("missing or ambiguous probe report")
    report = json.loads(reports[0].read_text(encoding="utf-8"))
    if report.get("result") != "ok":
        raise SystemExit(f"probe rejected: {report.get('reason', 'unknown reason')}")
    if len((report.get("strict_outcome") or {}).get("windows", [])) != args.windows:
        raise SystemExit("strict result does not match the requested window count")
    summary = analyze(output / "fragile-perf.csv",
                      (output / "probe.log").read_text(encoding="utf-8"),
                      report,
                      args.warmup_seconds, args.seconds)
    summary.update(workload=args.workload, evidence=str(output))
    content = json.dumps(summary, indent=2)
    (output / "summary.json").write_text(content + "\n", encoding="utf-8")
    print(content)


if __name__ == "__main__":
    main()
