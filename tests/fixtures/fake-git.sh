#!/bin/sh

case "${0##*/}" in
  wl-copy|xclip|xsel|pbcopy)
    if [ -n "${GD_TEST_CLIPBOARD_FAIL:-}" ]; then
      echo "provider display is unavailable" >&2
      exit 17
    fi
    cat > "$GD_TEST_CLIPBOARD_OUTPUT"
    exit 0
    ;;
  hash-fails)
    if [ "$1" = rev-parse ]; then
      exit 1
    fi
    printf 'hash failed\n' >&2
    exit 7
    ;;
  empty-tree-empty)
    if [ "$1" = rev-parse ]; then
      exit 1
    fi
    exit 0
    ;;
  empty-tree-non-utf8)
    if [ "$1" = rev-parse ]; then
      exit 1
    fi
    printf '\377'
    exit 0
    ;;
  hash-startup-fails)
    if [ "$1" = rev-parse ]; then
      rm "$0"
      exit 1
    fi
    ;;
  base-mid-startup-fails)
    if [ "$1" = remote ]; then
      rm "$0"
      exit 0
    fi
    ;;
  untracked-startup-fails)
    if [ "$1" = diff ]; then
      rm "$0"
      exit 0
    fi
    ;;
  untracked-command-fails)
    if [ "$1" = diff ]; then
      exit 0
    fi
    exit 9
    ;;
  diff-fails)
    exit 1
    ;;
  reference-non-utf8)
    printf '\377'
    exit 0
    ;;
esac

exit 1
