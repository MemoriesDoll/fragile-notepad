"""Measure software-first startup in isolated settings/cache directories."""

import argparse
import csv
import json
import math
import os
from pathlib import Path
import queue
import statistics
import subprocess
import tempfile
import threading
import time


def sample(binary, directory):
    env = dict(os.environ, FRAGILE_NOTEPAD_STARTUP_PROBE="1",
               APPDATA=str(directory / "config"), LOCALAPPDATA=str(directory / "local"),
               XDG_CONFIG_HOME=str(directory / "config"), XDG_CACHE_HOME=str(directory / "cache"),
               FRAGILE_PERF_TRACE="1", FRAGILE_PERF_TRACE_DIR=str(directory))
    events = queue.Queue()
    values = {}
    started = time.perf_counter()
    with (directory / "stderr.log").open("w", encoding="utf-8") as errors:
        process = subprocess.Popen([str(binary), "--no-session"], env=env,
                                   stdout=subprocess.PIPE, stderr=errors, text=True,
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)

        def read():
            for line in process.stdout:
                events.put(line.strip())
            events.put(None)

        reader = threading.Thread(target=read, daemon=True)
        reader.start()
        try:
            deadline = started + 15
            while "first_frame_ms" not in values:
                if time.perf_counter() >= deadline:
                    raise TimeoutError(f"Startup exceeded 15 seconds; see {directory}")
                line = events.get(timeout=max(0.001, deadline - time.perf_counter()))
                if line is None:
                    raise RuntimeError(f"Startup exited before first frame; see {directory}")
                for prefix, name in (("FRAGILE_NOTEPAD_FIRST_VIEW_READY_MS=", "first_view_ms"),
                                     ("FRAGILE_NOTEPAD_FIRST_FRAME_READY_MS=", "first_frame_ms")):
                    if line.startswith(prefix):
                        values[name] = float(line[len(prefix):])
            values["process_to_probe_ms"] = (time.perf_counter() - started) * 1000
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            reader.join(timeout=5)
            process.stdout.close()
    if "first_view_ms" not in values:
        raise RuntimeError("Missing first-view timing")
    with (directory / "fragile-perf.csv").open(encoding="utf-8", newline="") as trace:
        presents = [row for row in csv.DictReader(trace)
                    if row["event"] == "fallback_present" and "status=ok" in row["detail"]]
    if not presents or "backend=tiny-skia" not in presents[0]["detail"]:
        raise RuntimeError(f"Missing software-first presentation evidence; see {directory}")
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--output", type=Path, default=Path("target/startup-profile"))
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="run-", dir=args.output)).resolve()
    results = []
    for index in range(args.samples):
        directory = output / str(index)
        directory.mkdir()
        results.append(sample(binary, directory))
        print(f"STARTUP_SAMPLE index={index} {json.dumps(results[-1])}", flush=True)
    (output / "samples.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    for metric in results[0]:
        times = sorted(result[metric] for result in results)
        print(f"STARTUP_PROFILE metric={metric} median_ms={statistics.median(times):.3f} "
              f"p95_ms={times[math.ceil(len(times) * .95) - 1]:.3f}")
    print(f"Startup evidence: {output}")


if __name__ == "__main__":
    main()
