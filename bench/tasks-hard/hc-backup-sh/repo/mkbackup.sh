#!/bin/sh
# mkbackup.sh DIR NAME - create a timestamped tar backup file in DIR.
set -eu
dir="$1"; name="$2"
ts=$(date +%Y%m%d-%H%M%S)
mkdir -p "$dir"
: > "$dir/$name-$ts.tar"
