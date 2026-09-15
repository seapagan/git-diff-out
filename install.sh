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

main() {
    target=$(detect_target)
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' 0
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

    install_dir=${GD_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}
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
    case ":${PATH:-}:" in
        *":$install_dir:"*) ;;
        *) printf 'warning: %s is not on PATH; add it to PATH to use gd and git-diff-out.\n' "$install_dir" >&2 ;;
    esac
}

main "$@"
