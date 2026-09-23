#!/bin/sh
set -eu

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os:$arch" in
        Linux:x86_64|Linux:amd64) printf '%s\n' x86_64-unknown-linux-gnu ;;
        Darwin:x86_64|Darwin:amd64) printf '%s\n' x86_64-apple-darwin ;;
        Darwin:arm64|Darwin:aarch64) printf '%s\n' aarch64-apple-darwin ;;
        *) printf 'error: unsupported platform: %s/%s\n' "$os" "$arch" >&2; return 1 ;;
    esac
}

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        printf 'error: curl or wget is required\n' >&2
        return 1
    fi
}

normalize_dir() {
    value=$1
    while [ "$value" != / ] && [ "${value%/}" != "$value" ]; do
        value=${value%/}
    done
    printf '%s\n' "$value"
}

main() {
    target=$(detect_target)
    tmp_dir=$(mktemp -d)
    staged_man=
    cleanup() {
        if [ -n "$staged_man" ]; then
            rm -f "$staged_man" || :
        fi
        rm -rf "$tmp_dir" || :
    }
    trap 'cleanup' 0
    trap 'exit 1' 1 2 3 15

    version=${GD_VERSION:-}
    if [ -z "$version" ]; then
        download 'https://api.github.com/repos/seapagan/git-diff-out/releases/latest' "$tmp_dir/latest.json"
        version=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$tmp_dir/latest.json" | head -n 1)
        if [ -z "$version" ]; then
            printf 'error: latest release tag was not found\n' >&2
            return 1
        fi
    fi

    install_dir=$(normalize_dir "${GD_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}")
    man_dir=
    if [ "${GD_SKIP_MAN:-}" = 1 ]; then
        printf 'Skipped man page installation because GD_SKIP_MAN=1.\n'
    elif [ -n "${GD_MAN_DIR:-}" ]; then
        man_dir=$(normalize_dir "$GD_MAN_DIR")
    else
        case "$install_dir" in
            ?*/bin) man_dir=${install_dir%/bin}/share/man/man1 ;;
            *) printf 'Skipped man page installation: set GD_MAN_DIR for binary directory %s.\n' "$install_dir" ;;
        esac
    fi

    asset="git-diff-out-v${version}-${target}.tar.gz"
    download "https://github.com/seapagan/git-diff-out/releases/download/${version}/${asset}" "$tmp_dir/$asset"
    tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
    if [ ! -f "$tmp_dir/gd" ] || [ ! -f "$tmp_dir/git-diff-out" ]; then
        printf 'error: release archive is missing gd or git-diff-out\n' >&2
        return 1
    fi
    mkdir -p "$install_dir"
    install -m 755 "$tmp_dir/gd" "$tmp_dir/git-diff-out" "$install_dir/"
    printf 'Installed git-diff-out %s to %s (gd, git-diff-out).\n' "$version" "$install_dir"
    if [ -n "$man_dir" ]; then
        if [ ! -f "$tmp_dir/gd.1" ]; then
            printf 'Skipped man page installation: release archive does not contain gd.1.\n'
        elif mkdir -p "$man_dir" &&
            [ ! -d "$man_dir/gd.1" ] &&
            staged_man=$(mktemp "$man_dir/.gd.1.XXXXXX") &&
            install -m 644 "$tmp_dir/gd.1" "$staged_man" &&
            mv -f "$staged_man" "$man_dir/gd.1"
        then
            staged_man=
            printf 'Installed gd.1 to %s.\n' "$man_dir"
        else
            if [ -n "$staged_man" ]; then
                if rm -f "$staged_man"; then
                    staged_man=
                fi
            fi
            printf 'warning: could not install gd.1 to %s; binaries remain installed.\n' "$man_dir" >&2
        fi
    fi
    case ":${PATH:-}:" in
        *":$install_dir:"*) ;;
        *) printf 'warning: %s is not on PATH; add it to PATH to use gd and git-diff-out.\n' "$install_dir" >&2 ;;
    esac
}

main "$@"
