"""Summarize wgpu 29 RON uploads from profile_vulkan_resources API captures.

Mapped host writes and actual GPU copies are separate: a staging belt may map
far more memory than the commands copy. API writes precede the frame's marker;
encoded copies follow it. Tracing timings are intentionally not reported.
"""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re


def analyze(path, from_frame):
    source = path.read_text(encoding="utf-8")
    actions = list(re.finditer(r"^([A-Z]\w*)\(", source, re.M))
    buffers, textures = {}, {}
    queued = []
    frames = {}
    current = None
    totals = defaultdict(lambda: defaultdict(int))
    image_frames = []

    def record(label, metric, size):
        if current is not None and current[0] >= from_frame:
            totals[label][metric + "_count"] += 1
            totals[label][metric + "_bytes"] += size

    for index, action in enumerate(actions):
        text = source[action.start():actions[index + 1].start() if index + 1 < len(actions) else len(source)]
        kind = action[1]
        if kind in ("CreateBuffer", "CreateTexture"):
            identity = re.search(r"PointerId\((\d+)\)", text)[1]
            label = re.search(r'label: Some\("((?:\\.|[^"\\])*)"\)', text)
            (buffers if kind == "CreateBuffer" else textures)[identity] = label[1] if label else "unlabeled"
        elif kind == "WriteBuffer":
            identity = re.search(r"id: PointerId\((\d+)\)", text)[1]
            size = int(re.search(r"\bsize: (\d+)", text)[1])
            metric = "queue_buffer" if "queued: true" in text else "mapped_host"
            queued.append((buffers.get(identity, "unknown buffer"), metric, size))
        elif kind == "WriteTexture":
            identity = re.search(r"texture: PointerId\((\d+)\)", text)[1]
            data = re.search(r'data: File\("([^"\\]+)"\)', text)
            if not data:
                raise ValueError("Expected external WriteTexture payload in wgpu trace")
            queued.append((textures.get(identity, "unknown texture"), "queue_texture",
                           (path.parent / data[1]).stat().st_size))
        elif kind == "Submit":
            marker = re.search(r'InsertDebugMarker\("profile frame=(\d+) window=(\d+) scale=([\d.]+)"\)', text)
            if marker:
                current = (int(marker[1]), int(marker[2]), float(marker[3]))
                frames[current] = True
                for label, metric, size in queued:
                    record(label, metric, size)
                queued.clear()
            for copy in re.finditer(r"^    CopyBufferToBuffer\((.*?)^    \),", text, re.M | re.S):
                target = re.search(r"dst: PointerId\((\d+)\)", copy[1])[1]
                size = int(re.search(r"size: Some\((\d+)\)", copy[1])[1])
                record(buffers.get(target, "unknown buffer"), "buffer_copy", size)
            for copy in re.finditer(r"^    CopyBufferToTexture\((.*?)^    \),", text, re.M | re.S):
                target = re.search(r"texture: PointerId\((\d+)\)", copy[1])[1]
                label = textures.get(target, "unknown texture")
                row_bytes = int(re.search(r"bytes_per_row: Some\((\d+)\)", copy[1])[1])
                height = int(re.search(r"\bheight: (\d+)", copy[1])[1])
                layers = int(re.search(r"depthOrArrayLayers: (\d+)", copy[1])[1])
                record(label, "texture_copy_padded", row_bytes * height * layers)
                if current is not None and current[0] >= from_frame and "image texture atlas" in label:
                    image_frames.append(dict(frame=current[0], window=current[1], scale=current[2]))
    if not frames:
        raise ValueError("No profiler frame markers found")
    # Pending writes after the final marker cannot safely be attributed to it.
    if any(metric != "mapped_host" for _, metric, _ in queued):
        raise ValueError("Unattributed API writes after the final profiler frame")
    return dict(frames=sum(frame[0] >= from_frame for frame in frames),
                resources=dict(totals), image_upload_frames=image_frames,
                teardown_mapped_host_bytes=sum(size for _, _, size in queued))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--from-frame", type=int, default=0)
    args = parser.parse_args()
    print(json.dumps(analyze(args.trace, args.from_frame), indent=2))


if __name__ == "__main__":
    main()
