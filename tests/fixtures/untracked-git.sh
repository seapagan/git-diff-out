#!/bin/sh
if [ "$1" = --version ] && [ -n "${GD_TEST_GIT_VERSION:-}" ]; then
    printf '%s\n' "$GD_TEST_GIT_VERSION"
    exit "${GD_TEST_VERSION_EXIT:-0}"
fi
if [ "$1" = ls-tree ]; then
    if [ "${GD_TEST_TREE_POLICY:-}" = forbid ]; then
        printf 'unexpected revision enumeration\n' >&2
        exit 31
    fi
    if [ "${GD_TEST_TREE_POLICY:-}" = targeted ]; then
        if [ "$#" -ne 7 ] || [ "$2" != --full-tree ] || [ "$6" != -- ]; then
            printf 'expected a single targeted revision lookup\n' >&2
            exit 32
        fi
    fi
fi
exec "$GD_TEST_REAL_GIT" "$@"
