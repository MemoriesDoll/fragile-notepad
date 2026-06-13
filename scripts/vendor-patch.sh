#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/vendor-patch.sh <status|apply|export|refresh|update> --vendor-dir <path> --patch <path> --base-revision-file <path> [--remote <url>] [--revision <rev>] [--branch <branch>] [--git-config key=value] [--git-vendor] [--force]
EOF
}

command_name="${1:-}"
if [[ -z "$command_name" ]]; then
    usage
    exit 2
fi
shift

case "$command_name" in
    status|apply|export|refresh|update) ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        usage
        exit 2
        ;;
esac

vendor_dir=""
patch_path=""
base_revision_file=""
remote_url=""
revision=""
branch=""
git_config=()
git_config_count=0
git_vendor=0
force=0

vendor_git() {
    if [[ "$git_config_count" -gt 0 ]]; then
        git -C "$vendor_dir" "${git_config[@]}" "$@"
    else
        git -C "$vendor_dir" "$@"
    fi
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --vendor-dir)
            vendor_dir="${2:?missing value for --vendor-dir}"
            shift 2
            ;;
        --patch)
            patch_path="${2:?missing value for --patch}"
            shift 2
            ;;
        --base-revision-file)
            base_revision_file="${2:?missing value for --base-revision-file}"
            shift 2
            ;;
        --remote)
            remote_url="${2:?missing value for --remote}"
            shift 2
            ;;
        --revision)
            revision="${2:?missing value for --revision}"
            shift 2
            ;;
        --branch)
            branch="${2:?missing value for --branch}"
            shift 2
            ;;
        --git-config)
            git_config+=("-c" "${2:?missing value for --git-config}")
            git_config_count=$((git_config_count + 2))
            shift 2
            ;;
        --git-vendor)
            git_vendor=1
            shift
            ;;
        --force)
            force=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 2
            ;;
    esac
done

if [[ -z "$vendor_dir" || -z "$patch_path" || -z "$base_revision_file" ]]; then
    usage
    exit 2
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

assert_vendor_dir() {
    if [[ ! -d "$vendor_dir" ]]; then
        echo "Vendor directory not found: $vendor_dir" >&2
        exit 1
    fi
}

is_git_vendor() {
    [[ "$git_vendor" -eq 1 || -e "$vendor_dir/.git" ]]
}

assert_git_vendor() {
    if ! is_git_vendor; then
        echo "$vendor_dir is not a Git checkout. Vendor directories must be Git repositories." >&2
        exit 1
    fi
}

assert_clean_vendor() {
    local status
    assert_git_vendor
    status="$(vendor_git status --short)"

    if [[ -n "$status" && "$force" -ne 1 ]]; then
        echo "$vendor_dir has local changes:" >&2
        echo "$status" >&2
        echo "Use --force to allow applying over or exporting from a dirty vendor checkout." >&2
        exit 1
    fi
}

get_base_revision() {
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

write_base_revision() {
    mkdir -p "$(dirname "$base_revision_file")"
    printf '%s' "$1" > "$base_revision_file"
}

repo_path() {
    case "$1" in
        /*) printf '%s' "$1" ;;
        *) printf '%s/%s' "$repo_root" "$1" ;;
    esac
}

repo_relative_path() {
    local normalized root_normalized
    normalized="${1//\\//}"
    root_normalized="${repo_root//\\//}"

    case "$normalized" in
        "$root_normalized"/*)
            normalized="${normalized#"$root_normalized"/}"
            ;;
        ./*)
            normalized="${normalized#./}"
            ;;
    esac

    printf '%s' "$normalized"
}

patch_vendor_path() {
    local existing_prefix
    if existing_prefix="$(patch_prefix_from_existing_patch)"; then
        printf '%s' "$existing_prefix"
        return
    fi

    repo_relative_path "$(repo_path "$vendor_dir")"
}

superproject_vendor_path() {
    repo_relative_path "$(repo_path "$vendor_dir")"
}

patch_prefix_from_existing_patch() {
    local line relative i j rest prefix
    [[ -f "$patch_path" ]] || return 1

    line=""
    while IFS= read -r line; do
        [[ "$line" == diff\ --git\ a/*\ b/* ]] && break
        line=""
    done < "$patch_path"
    [[ -n "$line" ]] || return 1

    relative="${line#diff --git a/}"
    relative="${relative%% b/*}"

    IFS=/ read -r -a parts <<< "$relative"
    for ((i = 0; i < ${#parts[@]}; i++)); do
        rest="${parts[$i]}"
        for ((j = i + 1; j < ${#parts[@]}; j++)); do
            rest="$rest/${parts[$j]}"
        done

        if [[ -e "$vendor_dir/$rest" ]]; then
            if [[ "$i" -eq 0 ]]; then
                printf ''
            else
                prefix="${parts[0]}"
                for ((j = 1; j < i; j++)); do
                    prefix="$prefix/${parts[$j]}"
                done
                printf '%s' "$prefix"
            fi
            return 0
        fi
    done

    return 1
}

convert_git_vendor_patch() {
    local vendor_path
    vendor_path="$(patch_vendor_path)"
    sed \
        -e "s|^diff --git a/\\([^[:space:]]*\\) b/\\([^[:space:]]*\\)|diff --git a/$vendor_path/\\1 b/$vendor_path/\\2|" \
        -e "s|^--- a/|--- a/$vendor_path/|" \
        -e "s|^+++ b/|+++ b/$vendor_path/|"
}

vendor_head() {
    vendor_git rev-parse HEAD
}

superproject_gitlink() {
    local vendor_path mode hash stage path
    vendor_path="$(superproject_vendor_path)"
    while read -r mode hash stage path; do
        if [[ "$mode" == "160000" ]]; then
            printf '%s' "$hash"
            return
        fi
    done < <(git ls-files --stage -- "$vendor_path")
}

pin_consistency_problems() {
    local head base gitlink
    head="$(vendor_head)"
    base="$(get_base_revision)"
    gitlink="$(superproject_gitlink)"

    if [[ "$base" != "$head" ]]; then
        printf 'recorded base %s does not match vendor HEAD %s\n' "$base" "$head"
    fi
    if [[ -n "$gitlink" && "$gitlink" != "$head" ]]; then
        printf 'staged superproject gitlink %s does not match vendor HEAD %s\n' "$gitlink" "$head"
    fi
}

warn_pin_consistency() {
    local problems vendor_path
    problems="$(pin_consistency_problems)"
    if [[ -z "$problems" ]]; then
        return
    fi

    vendor_path="$(superproject_vendor_path)"
    echo "warning: $vendor_path is not fully reproducible yet:" >&2
    while IFS= read -r problem; do
        [[ -n "$problem" ]] && echo "warning:   $problem" >&2
    done <<< "$problems"
    echo "warning: Stage the matching files, for example: git add $base_revision_file $patch_path" >&2
}

export_patch() {
    assert_vendor_dir
    assert_git_vendor
    mkdir -p "$(dirname "$patch_path")"
    local temp_patch
    temp_patch="$(mktemp "${TMPDIR:-/tmp}/vendor-patch.XXXXXX")"
    local resolved_patch_path converted_patch
    resolved_patch_path="$(repo_path "$patch_path")"
    converted_patch="$(dirname "$resolved_patch_path")/.$(basename "$patch_path").$$.tmp"
    local -a untracked=()
    local untracked_count=0

    while IFS= read -r file; do
        untracked+=("$file")
        untracked_count=$((untracked_count + 1))
    done < <(vendor_git ls-files --others --exclude-standard)

    cleanup_export() {
        if [[ "$untracked_count" -gt 0 ]]; then
            vendor_git reset -q -- "${untracked[@]}" >/dev/null || true
        fi
        rm -f "$temp_patch" "$converted_patch"
        trap - RETURN
    }
    trap cleanup_export RETURN

    if [[ "$untracked_count" -gt 0 ]]; then
        vendor_git add -N -- "${untracked[@]}"
    fi

    vendor_git diff --binary --output="$temp_patch"
    convert_git_vendor_patch < "$temp_patch" > "$converted_patch"
    protect_patch_replacement "$resolved_patch_path" "$converted_patch"
    write_base_revision "$(vendor_head)"
    warn_pin_consistency
}

protect_patch_replacement() {
    local old_patch="$1"
    local new_patch="$2"
    local old_size=0
    local new_size=0

    if [[ -f "$old_patch" ]]; then
        old_size="$(wc -c < "$old_patch")"
    fi
    if [[ -f "$new_patch" ]]; then
        new_size="$(wc -c < "$new_patch")"
    fi

    if [[ "$old_size" -gt 0 && "$new_size" -eq 0 && "$force" -ne 1 ]]; then
        echo "Refusing to overwrite non-empty patch with an empty export: $patch_path. Use --force if this is intentional." >&2
        exit 1
    fi

    if [[ "$old_size" -gt 0 ]]; then
        local backup_dir timestamp
        backup_dir="$(dirname "$old_patch")/.backups"
        timestamp="$(date +%Y%m%d-%H%M%S)"
        mkdir -p "$backup_dir"
        cp "$old_patch" "$backup_dir/$(basename "$old_patch").$timestamp.bak"
    fi

    mv -f "$new_patch" "$old_patch"
}

apply_patch() {
    assert_vendor_dir
    assert_git_vendor
    assert_clean_vendor
    resolved_patch_path="$(repo_path "$patch_path")"
    vendor_git apply "-p$(git_vendor_strip_count)" "$resolved_patch_path"
}

reverse_patch() {
    assert_vendor_dir
    assert_git_vendor
    resolved_patch_path="$(repo_path "$patch_path")"
    vendor_git apply -R "-p$(git_vendor_strip_count)" "$resolved_patch_path"
}

vendor_has_changes() {
    [[ -n "$(vendor_git status --short)" ]]
}

resolve_update_revision() {
    target_branch="$branch"
    if [[ -z "$target_branch" ]]; then
        target_branch="HEAD"
    fi

    if [[ -n "$remote_url" ]]; then
        vendor_git fetch "$remote_url" "$target_branch"
        vendor_git rev-parse FETCH_HEAD
    else
        vendor_git fetch origin "$target_branch"
        if [[ "$target_branch" == "HEAD" ]]; then
            vendor_git rev-parse FETCH_HEAD
        else
            vendor_git rev-parse "origin/$target_branch"
        fi
    fi
}

git_vendor_strip_count() {
    local normalized parts
    normalized="$(patch_vendor_path)"
    if [[ -z "$normalized" ]]; then
        printf '1\n'
        return
    fi

    IFS=/ read -r -a parts <<< "$normalized"
    printf '%s\n' "$((${#parts[@]} + 1))"
}

case "$command_name" in
    status)
        assert_vendor_dir
        assert_git_vendor
        echo "vendor dir:       $vendor_dir"
        echo "recorded base:    $(get_base_revision)"
        echo "patch:            $patch_path"
        echo "vendor HEAD:      $(vendor_head)"
        gitlink="$(superproject_gitlink)"
        if [[ -n "$gitlink" ]]; then
            echo "superproject pin: $gitlink"
        else
            echo "superproject pin: <script-managed checkout>"
        fi
        pin_problems="$(pin_consistency_problems)"
        if [[ -n "$pin_problems" ]]; then
            echo "pin status:       mismatch"
            while IFS= read -r problem; do
                [[ -n "$problem" ]] && echo "  - $problem"
            done <<< "$pin_problems"
        else
            echo "pin status:       ok"
        fi
        echo
        vendor_git status --short
        ;;
    apply)
        apply_patch
        ;;
    export)
        export_patch
        ;;
    refresh)
        assert_vendor_dir
        assert_git_vendor
        assert_clean_vendor

        target_revision="$revision"
        if [[ -z "$target_revision" ]]; then
            target_revision="$(get_base_revision)"
        fi

        if [[ -n "$remote_url" ]]; then
            vendor_git remote set-url origin "$remote_url"
            vendor_git fetch origin
        fi

        vendor_git checkout "$target_revision"
        write_base_revision "$target_revision"
        apply_patch
        ;;
    update)
        assert_vendor_dir
        assert_git_vendor

        target_revision="$(resolve_update_revision)"

        if vendor_has_changes; then
            export_patch
            reverse_patch
            assert_clean_vendor
        fi

        vendor_git checkout "$target_revision"
        write_base_revision "$target_revision"
        apply_patch
        export_patch
        ;;
esac
