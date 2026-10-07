#!/bin/sh
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
# verify.sh sanity: good names pass, bad names fail
mkdir -p "$tmp/g" "$tmp/bad"
: > "$tmp/g/app-20250101-010203.tar"
: > "$tmp/bad/junk.tar"
sh verify.sh "$tmp/g"
if sh verify.sh "$tmp/bad" 2>/dev/null; then echo "FAIL: bad name accepted"; exit 1; fi
echo "verify tests passed"
