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

if [[ "$skip_submodule_update" -eq 1 ]]; then
    echo "warning: --skip-submodule-update is deprecated; vendor checkouts are managed by this script." >&2
fi

get_base_revision() {
    local base_revision_file="$1"
    if [[ ! -f "$base_revision_file" ]]; then
        echo "Base revision file not found: $base_revision_file" >&2
        exit 1
    fi

    local value
    IFS= read -r value < "$base_revision_file" || true
    value="${value//$'\r'/}"
    value="${value//$'\n'/}"
    printf '%s' "$value"
}

is_git_checkout() {
    [[ -e "$1/.git" ]]
}

ensure_vendor_checkout() {
    local vendor_dir="$1"
    local remote_url="$2"
    local base_revision_file="$3"
    shift 3

    if [[ -e "$vendor_dir" ]]; then
        if ! is_git_checkout "$vendor_dir"; then
            echo "$vendor_dir exists but is not a Git checkout." >&2
            exit 1
        fi

        return
    fi

    mkdir -p "$(dirname "$vendor_dir")"
    local base_revision
    base_revision="$(get_base_revision "$base_revision_file")"

    git clone --no-checkout "$remote_url" "$vendor_dir"
    local entry
    while [[ "$#" -gt 0 ]]; do
        entry="$1"
        shift
        if [[ "$entry" != *=* ]]; then
            echo "Invalid Git config entry for $vendor_dir: $entry" >&2
            exit 1
        fi
        git -C "$vendor_dir" config "${entry%%=*}" "${entry#*=}"
    done
    git -C "$vendor_dir" checkout "$base_revision"
}

missing_vendor_status() {
    local vendor_dir="$1"
    local patch_path="$2"
    local base_revision_file="$3"

    echo "vendor dir:       $vendor_dir"
    if [[ -f "$base_revision_file" ]]; then
        echo "recorded base:    $(get_base_revision "$base_revision_file")"
    else
        echo "recorded base:    <missing>"
    fi
    echo "patch:            $patch_path"
    echo "checkout status:  missing; run setup-vendor apply to bootstrap"
    echo
}

if [[ "$command_name" == "apply" || "$command_name" == "update" ]]; then
    ensure_vendor_checkout vendor/iced https://github.com/iced-rs/iced.git patches/iced/BASE_REVISION core.autocrlf=true
    ensure_vendor_checkout vendor/encoding_rs https://github.com/hsivonen/encoding_rs.git patches/encoding_rs/BASE_REVISION
fi

if [[ "$command_name" == "status" && ! -e vendor/iced ]]; then
    missing_vendor_status vendor/iced patches/iced/fragile-notepad-iced.patch patches/iced/BASE_REVISION
elif [[ "$command_name" == "update" ]]; then
    "${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
        --vendor-dir vendor/iced \
        --patch patches/iced/fragile-notepad-iced.patch \
        --base-revision-file patches/iced/BASE_REVISION \
        --git-vendor \
        --git-config core.autocrlf=true \
        --remote https://github.com/iced-rs/iced.git \
        --branch master
else
    "${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
        --vendor-dir vendor/iced \
        --patch patches/iced/fragile-notepad-iced.patch \
        --base-revision-file patches/iced/BASE_REVISION \
        --git-vendor \
        --git-config core.autocrlf=true
fi

if [[ "$command_name" == "status" && ! -e vendor/encoding_rs ]]; then
    missing_vendor_status vendor/encoding_rs patches/encoding_rs/oem-code-pages.patch patches/encoding_rs/BASE_REVISION
elif [[ "$command_name" == "update" ]]; then
    "${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
        --vendor-dir vendor/encoding_rs \
        --patch patches/encoding_rs/oem-code-pages.patch \
        --base-revision-file patches/encoding_rs/BASE_REVISION \
        --git-vendor \
        --remote https://github.com/hsivonen/encoding_rs.git \
        --branch main
else
    "${BASH:-bash}" scripts/vendor-patch.sh "$command_name" \
        --vendor-dir vendor/encoding_rs \
        --patch patches/encoding_rs/oem-code-pages.patch \
        --base-revision-file patches/encoding_rs/BASE_REVISION \
        --git-vendor
fi
