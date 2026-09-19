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
  repository-invalid-utf8-then-valid)
    if [ "$1" = remote ] && [ "$2" = get-url ]; then
      case "$3" in
        broken) printf '\377' ;;
        backup) printf 'git@example.com:group/project.git\n' ;;
        *) exit 1 ;;
      esac
      exit 0
    fi
    if [ "$1" = remote ]; then
      printf 'broken\nbackup\n'
      exit 0
    fi
    if [ "$1" = symbolic-ref ]; then
      exit 1
    fi
    ;;
  repository-invalid-utf8-fallback)
    if [ "$1" = remote ] && [ "$2" = get-url ]; then
      if [ "$3" = broken ]; then
        printf '\377'
        exit 0
      fi
      exit 1
    fi
    if [ "$1" = remote ]; then
      printf 'broken\n'
      exit 0
    fi
    if [ "$1" = symbolic-ref ]; then
      exit 1
    fi
    if [ "$1" = rev-parse ]; then
      printf '%s\n' "$PWD"
      exit 0
    fi
    ;;
  repository-get-url-startup-fails)
    if [ "$1" = remote ] && [ "$2" = get-url ]; then
      rm "$0"
      exit 1
    fi
    if [ "$1" = remote ]; then
      printf 'backup\n'
      exit 0
    fi
    if [ "$1" = symbolic-ref ]; then
      exit 1
    fi
    ;;
esac

exit 1
