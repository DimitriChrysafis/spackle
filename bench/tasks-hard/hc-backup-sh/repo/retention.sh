#!/bin/sh
# retention.sh DIR - print files older than RETENTION_DAYS (default 30).
set -eu
dir="$1"
days="${RETENTION_DAYS:-30}"
find "$dir" -name '*.tar' -type f -mtime +"$days" -print
