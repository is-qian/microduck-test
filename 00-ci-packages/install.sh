#!/bin/sh
# Vendored packages for the CI build container: when an upstream download
# breaks (e.g. the runner's route to ziglang.org), drop the file here and
# it gets installed with no other change:
#   *.tar.*  extracted into /usr/local (single top-level dir, stripped)
#   *.deb    installed with dpkg
# build-dist.sh runs this before the tool builds (the 00- prefix sorts the
# directory first; board-bundle needs zig). Nothing is staged into the
# dist tarball, so these packages never ship to the board.
set -e
cd "$(dirname "$0")"
for f in ./*.tar.*; do
    [ -e "$f" ] || continue
    tar -xf "$f" -C /usr/local --strip-components=1
done
for f in ./*.deb; do
    [ -e "$f" ] || continue
    dpkg -i "$f"
done
