#!/bin/sh
# verify.sh DIR - sanity-check backups in DIR: all *.tar files must
# have parseable NAME-YYYYMMDD-HHMMSS.tar names. Exit 1 on bad names.
set -eu
dir="$1"
bad=0
for f in "$dir"/*.tar; do
    [ -e "$f" ] || continue
    base=${f##*/}
    stamp=${base##*-}
    stamp=${base%.*}
    stamp=${stamp##*-}
    case "$stamp" in
        *[!0-9]*|'') echo "bad name: $f" >&2; bad=1 ;;
    esac
done
[ "$bad" -eq 0 ]
