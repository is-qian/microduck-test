#!/bin/sh
# Dist build entry (see ../build-dist.sh): build and install the runtime
# payload into $1. Not shipped in the payload, so the image chroot never
# runs this; the board gets the prebuilt binary only.
set -e
cd "$(dirname "$0")"
./build.sh
install -D -m 755 dxl "$1/dxl"
install -m 644 motor.conf README.md "$1/"
