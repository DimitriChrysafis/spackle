# bakrotate

Tiny backup rotation: `rotate.sh DIR KEEP` keeps the newest KEEP `*.tar`
files in DIR and removes older ones. `mkbackup.sh DIR NAME` writes a new
timestamped backup. Requires POSIX sh, find, sort, tail, rm.
