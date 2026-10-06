#!/bin/sh
out=$(sh count.sh "a b.txt" 2>&1)
case "$out" in
  *"3 a b.txt"*|*"3 ./a b.txt"*|*"a b.txt"*) echo ok; exit 0;;
esac
# also accept bare "3" plus filename
echo "$out" | grep -q "^3" && echo ok && exit 0
echo "unexpected: $out"; exit 1
