#!/bin/sh
for f in $@; do
  wc -l $f
done
