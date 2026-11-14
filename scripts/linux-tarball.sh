#!/usr/bin/env bash
# The tar.gz of a release: the program, the launcher entry and the icon the .deb installs (taken
# from the deb, so the three agree), the licenses and INSTALL.txt, which says where to put them.
#
#   scripts/linux-tarball.sh <the .deb> <the binary> <the .tar.gz to write>
set -eu
deb=${1:?usage: linux-tarball.sh <deb> <binary> <out.tar.gz>}
binary=${2:?usage: linux-tarball.sh <deb> <binary> <out.tar.gz>}
out=${3:?usage: linux-tarball.sh <deb> <binary> <out.tar.gz>}
out=$(cd "$(dirname "$out")" && pwd)/$(basename "$out")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
dpkg-deb --fsys-tarfile "$deb" |
  tar -x -C "$work" ./usr/share/applications/takeshot.desktop ./usr/share/icons/hicolor/256x256/apps/takeshot.png
cp "$binary" "$work/takeshot"
mv "$work/usr/share/applications/takeshot.desktop" "$work/takeshot.desktop"
mv "$work/usr/share/icons/hicolor/256x256/apps/takeshot.png" "$work/takeshot.png"
cp LICENSE THIRD_PARTY_NOTICES.md packaging/linux/INSTALL.txt "$work/"
tar -czf "$out" -C "$work" takeshot takeshot.desktop takeshot.png INSTALL.txt LICENSE THIRD_PARTY_NOTICES.md
