#!/bin/sh
# usage: backup.sh SRC DST
src="$1"
cp "$src" backup.out
echo copied
