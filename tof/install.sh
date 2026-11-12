#!/bin/sh
# Dist build entry (see ../build-dist.sh): build and install the runtime
# payload into $1. Not shipped in the payload, so the image chroot never
# runs this; the board gets the prebuilt binary only.
set -e
cd "$(dirname "$0")"
make CC=aarch64-linux-gnu-gcc CFLAGS="-O2 -Wall -static"
install -D -m 755 vl53l8cx-test "$1/vl53l8cx-test"
install -m 644 README.md "$1/"
