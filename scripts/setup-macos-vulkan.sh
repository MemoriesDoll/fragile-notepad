#!/usr/bin/env bash
# Source this in the same shell that starts Cargo (macOS SIP can strip DYLD_*
# when launching a new system shell). Install dependencies with:
# brew install molten-vk vulkan-loader vulkan-tools
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
    echo "This helper requires macOS." >&2
    return 1 2>/dev/null || exit 1
fi

fragile_vulkan_loader="$(brew --prefix vulkan-loader)"
fragile_molten_vk="$(brew --prefix molten-vk)"
test -r "$fragile_vulkan_loader/lib/libvulkan.dylib"
test -r "$fragile_molten_vk/etc/vulkan/icd.d/MoltenVK_icd.json"
export DYLD_LIBRARY_PATH="$fragile_vulkan_loader/lib:$fragile_molten_vk/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
export VK_DRIVER_FILES="$fragile_molten_vk/etc/vulkan/icd.d/MoltenVK_icd.json"
export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
export WGPU_BACKEND=vulkan
unset fragile_vulkan_loader fragile_molten_vk
