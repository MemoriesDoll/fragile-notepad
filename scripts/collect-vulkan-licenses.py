"""Collect notices from the exact sources named by installed Homebrew formulae."""

import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
from urllib.request import urlopen


def sources(name, formula):
    # Read the installed .brew formula, not today's potentially newer formula.
    stable = formula.read_text(encoding="utf-8").split("\n  head do", 1)[0]
    url = re.search(r'^\s*url "(https://github.com/[^" ]+\.tar\.gz)"', stable, re.M)
    checksum = re.search(r'^\s*sha256 "([0-9a-f]{64})"', stable, re.M)
    if not url or not checksum:
        raise ValueError(f"Unsupported source metadata in {formula}")
    yield name, url[1], checksum[1]
    for resource in re.finditer(r'resource "([\w-]+)" do(.*?)\n    end', stable, re.S):
        repository = re.search(r'url "https://github.com/([\w.-]+/[\w.-]+)\.git"', resource[2])
        revision = re.search(r'revision: "([0-9a-f]{40})"', resource[2])
        if not repository or not revision:
            raise ValueError(f"Unpinned resource {resource[1]} in {formula}")
        yield f"{name}-{resource[1]}", f"https://codeload.github.com/{repository[1]}/tar.gz/{revision[1]}", None


def collect(name, url, expected, output):
    with urlopen(url, timeout=60) as response:
        data = response.read(100 * 1024 * 1024 + 1)
    if len(data) > 100 * 1024 * 1024:
        raise ValueError(f"Oversized source archive: {name}")
    checksum = hashlib.sha256(data).hexdigest()
    if expected and checksum != expected:
        raise ValueError(f"Source checksum mismatch: {name}")
    notices = []
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive:
            path = PurePosixPath(member.name)
            if not member.isfile() or len(path.parts) < 2:
                continue
            relative = PurePosixPath(*path.parts[1:])
            if not (relative.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))
                    or "LICENSES" in [part.upper() for part in relative.parts]):
                continue
            if path.is_absolute() or ".." in path.parts or member.size > 2 * 1024 * 1024:
                raise ValueError(f"Unsafe notice path or size: {member.name}")
            destination = output / name / Path(*relative.parts)
            destination.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source:
                destination.write_bytes(source.read())
            notices.append(str(relative))
    if not notices:
        raise ValueError(f"No notices in {name}")
    return dict(component=name, source=url, sha256=checksum, notices=sorted(notices))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--loader-formula", type=Path, required=True)
    parser.add_argument("--molten-formula", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    records = []
    for name, formula in (("vulkan-loader", args.loader_formula), ("molten-vk", args.molten_formula)):
        for source in sources(name, formula):
            records.append(collect(*source, args.output))
            print(f"Collected notices: {source[0]}", flush=True)
    (args.output / "vulkan-sources.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
