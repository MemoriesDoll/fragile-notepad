#!/bin/bash
set -euo pipefail
fragile_package="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# Keep Vulkan lazy: configuring lookup does not load the driver. The executable
# still starts with tiny-skia and prepares Vulkan asynchronously when needed.
export DYLD_LIBRARY_PATH="$fragile_package/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
export VK_DRIVER_FILES="$fragile_package/share/vulkan/icd.d/MoltenVK_icd.json"
export VK_ICD_FILENAMES="$VK_DRIVER_FILES"
exec "$fragile_package/libexec/fragile-notepad" "$@"
