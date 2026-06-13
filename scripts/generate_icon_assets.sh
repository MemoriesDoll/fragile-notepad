#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

python3 scripts/generate_tango_rgba.py
python3 scripts/rasterize_svg_icons.py --svg-dir assets/icons/heroicons/svg --out-dir assets/icons/heroicons/rgba --size 22
python3 scripts/rasterize_svg_icons.py --svg-dir assets/icons/bootstrap/svg --out-dir assets/icons/bootstrap/rgba --size 22
