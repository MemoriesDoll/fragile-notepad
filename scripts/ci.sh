#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

cargo fmt --package fragile-notepad --check
bash scripts/generate_icon_assets.sh
python3 scripts/test_icon_assets.py
python3 scripts/test_profile_vulkan_live.py
python3 scripts/test_vulkan_handoff_evidence.py
cargo check

if [[ "$(uname -s)" == "Linux" ]] && command -v xvfb-run >/dev/null 2>&1; then
    WINIT_UNIX_BACKEND=x11 xvfb-run -a cargo test
else
    cargo test
fi

cargo test --locked -p iced_wgpu --lib
cargo test --locked -p cryoglyph --lib

cargo check --no-default-features
cargo check --examples
