#!/bin/sh
# rotate.sh DIR KEEP - keep the newest KEEP backup files in DIR,
# delete the rest. Backups are named NAME-YYYYMMDD-HHMMSS.tar
# sorted by name so the newest sort last... we keep last KEEP.
set -eu

dir="$1"
keep="$2"

[ -d "$dir" ] || { echo "no such dir: $dir" >&2; exit 2; }

count=$(find "$dir" -name '*.tar' -type f | wc -l | tr -d ' ')
[ "$count" -le "$keep" ] && exit 0

# delete all but the newest $keep
to_delete=$((count - keep))
find "$dir" -name '*.tar' -type f | sort | tail -n "$to_delete" |
    while IFS= read -r f; do rm "$f"; done
