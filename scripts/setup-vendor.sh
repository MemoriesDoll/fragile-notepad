#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/setup-vendor.sh [apply|status|update] [--skip-submodule-update]
EOF
}

command_name="apply"
skip_submodule_update=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        apply|status|update)
            command_name="$1"
            shift
            ;;
        --skip-submodule-update)
            skip_submodule_update=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

initialize_submodules() {
    if [[ "$#" -eq 0 ]]; then
        return
    fi

    git submodule update --init "$@"
}

if [[ "$skip_submodule_update" -ne 1 ]]; then
    if [[ "$command_name" == "apply" ]]; then
        initialize_submodules vendor/iced vendor/encoding_rs
    elif [[ "$command_name" == "update" ]]; then
        missing_submodules=()
        [[ -e vendor/iced ]] || missing_submodules+=(vendor/iced)
        [[ -e vendor/encoding_rs ]] || missing_submodules+=(vendor/encoding_rs)
        initialize_submodules "${missing_submodules[@]}"
    fi
fi

iced_update_args=()
encoding_update_args=()

if [[ "$command_name" == "update" ]]; then
    iced_update_args=(--remote https://github.com/iced-rs/iced.git --branch master)
    encoding_update_args=(--remote https://github.com/hsivonen/encoding_rs.git --branch main)
fi

"${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
    --vendor-dir vendor/iced \
    --patch patches/iced/fragile-notepad-iced.patch \
    --base-revision-file patches/iced/BASE_REVISION \
    --git-vendor \
    --git-config core.autocrlf=true \
    "${iced_update_args[@]}"

"${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
    --vendor-dir vendor/encoding_rs \
    --patch patches/encoding_rs/oem-code-pages.patch \
    --base-revision-file patches/encoding_rs/BASE_REVISION \
    --git-vendor \
    "${encoding_update_args[@]}"
