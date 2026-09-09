#!/bin/sh
# Install the RustQEC v0.3.0 native CLI without requiring a source checkout.

set -eu

VERSION=v0.3.0
RELEASE_BASE=https://github.com/nzy1997/rust-qec/releases/download/$VERSION
DEFAULT_BIN_DIR=${HOME:+$HOME/.local/bin}

usage() {
    cat <<'EOF'
Usage: install.sh [--bin-dir DIRECTORY]

Install RustQEC v0.3.0 (`rustqec` and `rstim`) into ~/.local/bin by default.

Supported platforms:
  Ubuntu 24.04 x86_64
  macOS 15 Apple silicon (arm64)

Options:
  --bin-dir DIRECTORY  Install into DIRECTORY instead of ~/.local/bin.
  --help               Show this help.

The installer does not use sudo, edit shell profiles, or replace existing
rustqec or rstim files. Choose another --bin-dir if either path is occupied.
An interrupted installation can leave already-installed complete binaries in
place; choose another --bin-dir to retry.
EOF
}

fail() {
    printf '%s\n' "error: $*" >&2
    exit 1
}

shell_quote() {
    # A single-quoted shell word, safe to paste into the PATH command below.
    printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

# Keep execution at the end so a truncated piped download cannot start installing.
main() {
    bin_dir=$DEFAULT_BIN_DIR
    while [ "$#" -gt 0 ]; do
        case $1 in
            --bin-dir)
                [ "$#" -ge 2 ] || fail '--bin-dir requires a directory'
                bin_dir=$2
                shift 2
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                fail "unknown option: $1 (run with --help)"
                ;;
        esac
    done

    [ -n "${bin_dir:-}" ] || fail 'HOME is not set; pass --bin-dir DIRECTORY'
    case $bin_dir in
        /*) ;;
        *) bin_dir=$(pwd -P)/$bin_dir ;;
    esac

    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)
            target=x86_64-unknown-linux-gnu
            expected_sha256=3ebafcd684f06478ee9d6f4fc85da61e10de4e498020b86c0e76806b67c35aff
            ;;
        Darwin-arm64|Darwin-aarch64)
            target=aarch64-apple-darwin
            expected_sha256=a74da165d4cd562aaa9e9cbc06d0a42bdf508e65bf283868df1dede4f0a6e8f7
            ;;
        *)
            fail "no validated native archive for $(uname -s) $(uname -m); supported: Linux x86_64 and macOS arm64"
            ;;
    esac

    # Refuse an occupied destination before downloading anything. This also avoids
    # replacing symlinks whose target might be outside the requested directory.
    for command in rustqec rstim; do
        destination=$bin_dir/$command
        if [ -e "$destination" ] || [ -L "$destination" ]; then
            fail "$destination already exists; choose another --bin-dir (existing files are never replaced)"
        fi
    done

    archive=rustqec-$VERSION-$target.tar.gz
    archive_root=${archive%.tar.gz}
    temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/rustqec-install.XXXXXX") || fail 'could not create a temporary directory'
    stage_dir=
    committed=0
    cleanup() {
        if [ "$committed" -ne 1 ] && [ -n "$stage_dir" ]; then
            printf '%s\n' "warning: installation stopped before completion; any published binaries were left in $bin_dir. Choose another --bin-dir to retry." >&2
        fi
        [ -z "$stage_dir" ] || rm -rf "$stage_dir"
        rm -rf "$temp_dir"
    }
    trap cleanup 0
    trap 'exit 1' HUP INT TERM

    printf 'Downloading RustQEC %s for %s...\n' "$VERSION" "$target"
    curl -fsSL "$RELEASE_BASE/$archive" -o "$temp_dir/$archive" || fail 'download failed'

    if command -v sha256sum >/dev/null 2>&1; then
        actual_sha256=$(sha256sum "$temp_dir/$archive" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual_sha256=$(shasum -a 256 "$temp_dir/$archive" | awk '{print $1}')
    else
        fail 'need sha256sum (Linux) or shasum (macOS) to verify the download'
    fi
    [ "$actual_sha256" = "$expected_sha256" ] || fail 'download checksum did not match; no files were installed'

    tar -xzf "$temp_dir/$archive" -C "$temp_dir" || fail 'could not unpack the verified archive'
    for command in rustqec rstim; do
        [ -f "$temp_dir/$archive_root/bin/$command" ] || fail "archive is missing $command"
        [ -x "$temp_dir/$archive_root/bin/$command" ] || fail "archive $command is not executable"
    done
    "$temp_dir/$archive_root/bin/rustqec" capabilities --format json >/dev/null 2>&1 || \
        fail 'the verified rustqec binary cannot run on this system; no files were installed'

    mkdir -p "$bin_dir" || fail "could not create $bin_dir"
    bin_dir=$(CDPATH= cd "$bin_dir" && pwd -P) || fail "could not resolve $bin_dir"
    stage_dir=$(mktemp -d "$bin_dir/.rustqec-install.XXXXXX") || fail 'could not create an installation staging directory'
    for command in rustqec rstim; do
        cp "$temp_dir/$archive_root/bin/$command" "$stage_dir/$command" || {
            fail "could not prepare $command for installation"
        }
        chmod 755 "$stage_dir/$command" || {
            fail "could not mark $command executable"
        }
    done

    # A hard link into a directory fails if its basename already exists. That
    # makes publication no-clobber even when another process races this script.
    for command in rustqec rstim; do
        ln "$stage_dir/$command" "$bin_dir/" || \
            fail "$bin_dir/$command appeared during installation; any earlier binary was left in place"
    done
    committed=1

    printf 'Installed rustqec and rstim in %s\n' "$bin_dir"
    case :${PATH:-}: in
        *:"$bin_dir":*) ;;
        *)
            printf 'Add it to this shell with:\n  export PATH=%s:$PATH\n' "$(shell_quote "$bin_dir")"
            ;;
    esac
}

main "$@"
