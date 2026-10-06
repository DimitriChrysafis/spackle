#!/bin/sh
rm -f dest.txt backup.out
sh backup.sh input.txt dest.txt >/dev/null
[ -f dest.txt ] && cmp -s input.txt dest.txt && echo ok || { echo "dest.txt missing or wrong"; exit 1; }
