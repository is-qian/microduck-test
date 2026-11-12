#!/bin/sh
# Dist build entry (see ../build-dist.sh): build and install the runtime
# payload into $1. Not shipped in the payload, so the image chroot never
# runs this; the board gets the prebuilt binaries only.
set -e
cd "$(dirname "$0")"
CC=aarch64-linux-gnu-gcc ./build.sh
install -D -m 755 nfc-test "$1/nfc-test"
install -m 755 nfc-id-test "$1/nfc-id-test"
install -m 644 README.md "$1/"
