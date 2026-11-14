#!/usr/bin/env bash
# The glibc a Linux binary needs (the highest GLIBC_x.y version it references) must be the floor
# the deb declares (`libc6 (>= x.y)` in Cargo.toml). A declared floor above it leaves out systems
# that could run the file; one below it lets apt install a file that then does not start.
#
#   scripts/glibc-floor.sh <binary> [Cargo.toml]
set -eu
binary=${1:?usage: glibc-floor.sh <binary> [Cargo.toml]}
manifest=${2:-Cargo.toml}
needed=$(objdump -T "$binary" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -n 1)
declared=$(sed -n 's/.*"libc6 (>= \([0-9][0-9.]*\))".*/\1/p' "$manifest" | head -n 1)
if [ -z "$needed" ] || [ -z "$declared" ]; then
  echo "glibc floor: could not read it (binary: '$needed', $manifest: '$declared')" >&2
  exit 1
fi
if [ "$needed" != "$declared" ]; then
  echo "glibc floor: the binary needs glibc $needed but $manifest declares libc6 (>= $declared)" >&2
  exit 1
fi
echo "glibc floor: $needed"
