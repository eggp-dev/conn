#!/bin/sh
# Exercise the documented one-file scenario without Conn or a model account.
# The fixture is deliberately left in a newly created temporary directory.
set -eu

conn_demo_dir=$(mktemp -d "${TMPDIR:-/tmp}/conn-demo-check.XXXXXX")
mkdir "$conn_demo_dir/draft" "$conn_demo_dir/workspace"
cd "$conn_demo_dir/draft"
test "${PWD##*/}" = draft

# Human correction in the shared shell.
cd ../workspace
test "${PWD##*/}" = workspace

# Agent continuation, after observing and checking the new directory.
(set -C; printf '%s\n' 'We continued from your correction.' > collaboration.txt)
test "$(cat collaboration.txt)" = 'We continued from your correction.'
test "$(wc -l < collaboration.txt | tr -d ' ')" = 1
test ! -e ../draft/collaboration.txt

# A retry must not replace the existing result.
if (set -C; printf '%s\n' 'This must never replace the result.' > collaboration.txt) 2>/dev/null; then
    printf '%s\n' 'FAIL: the second write replaced an existing file.' >&2
    exit 1
fi
test "$(cat collaboration.txt)" = 'We continued from your correction.'

printf '%s\n' 'PASS: corrected destination, exact one-line result, original destination empty, existing file preserved.'
printf 'Fixture: %s\n' "$conn_demo_dir"
