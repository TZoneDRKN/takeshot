#!/usr/bin/env bash
# The tar.gz of a release: the program, the launcher entry and the icon the .deb installs (taken
# from the deb, so the three agree), the licenses and INSTALL.txt, which says where to put them.
#
#   scripts/linux-tarball.sh <the .deb> <the binary> <the .tar.gz to write>
#
# Run from the root of the repository (LICENSE, THIRD_PARTY_NOTICES.md and packaging/linux/INSTALL.txt
# are read from there).
set -eu
if [ $# -ne 3 ]; then
  echo "usage: linux-tarball.sh <deb> <binary> <out.tar.gz> (got $# arguments)" >&2
  exit 2
fi
deb=$1
binary=$2
out=$3
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dpkg-deb --fsys-tarfile "$deb" |
  tar -x -C "$work" --wildcards '*usr/share/applications/takeshot.desktop' '*usr/share/icons/hicolor/256x256/apps/takeshot.png'
cp "$binary" "$work/takeshot"
mv "$work/usr/share/applications/takeshot.desktop" "$work/takeshot.desktop"
mv "$work/usr/share/icons/hicolor/256x256/apps/takeshot.png" "$work/takeshot.png"
cp LICENSE THIRD_PARTY_NOTICES.md packaging/linux/INSTALL.txt "$work/"
tar -czf "$out" -C "$work" takeshot takeshot.desktop takeshot.png INSTALL.txt LICENSE THIRD_PARTY_NOTICES.md
