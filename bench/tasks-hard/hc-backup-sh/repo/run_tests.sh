#!/bin/sh
set -eu
cd "$(dirname "$0")"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mk() { # mk DIR name seconds_ago -> timestamped backup file in the past
    d="$1"; n="$2"; ago="$3"
    ts=$(date -v-"${ago}S" +%Y%m%d-%H%M%S 2>/dev/null || date -d "-${ago} sec" +%Y%m%d-%H%M%S)
    : > "$d/$n-$ts.tar"
}

fail=0
check() { # check DESC EXPR...
    desc="$1"; shift
    if "$@"; then echo "ok: $desc"; else echo "FAIL: $desc"; fail=1; fi
}

mkdir -p "$tmp/a"
mk "$tmp/a" db 300; f300=$(find "$tmp/a" -name 'db-*' | sort | tail -1)
mk "$tmp/a" db 200; f200=$(find "$tmp/a" -name 'db-*' | sort | tail -1)
mk "$tmp/a" db 100; f100=$(find "$tmp/a" -name 'db-*' | sort | tail -1)
mk "$tmp/a" db 5;   f5=$(find "$tmp/a" -name 'db-*'   | sort | tail -1)

sh rotate.sh "$tmp/a" 2

left=$(find "$tmp/a" -name '*.tar' | wc -l | tr -d ' ')
check "keeps KEEP files" [ "$left" -eq 2 ]
check "newest survives"  [ -f "$f5" ]
check "second-newest survives" [ -f "$f100" ]
check "oldest deleted"  [ ! -f "$f300" ]
check "second-oldest deleted" [ ! -f "$f200" ]

mkdir -p "$tmp/b"
mk "$tmp/b" x 10
sh rotate.sh "$tmp/b" 3
check "under limit keeps all" [ "$(find "$tmp/b" -name '*.tar' | wc -l | tr -d ' ')" -eq 1 ]

if sh rotate.sh "$tmp/nope" 2 2>/dev/null; then
    echo "FAIL: missing dir should exit nonzero"; fail=1
fi

[ "$fail" -eq 0 ] || { echo "tests failed"; exit 1; }
echo "all tests passed"
