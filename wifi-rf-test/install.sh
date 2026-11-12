#!/bin/sh
# Two modes (see ../build-dist.sh and ../microduck-test-tools.sh):
#   install.sh <dir>   dist build: stage the runtime payload into <dir>
#   install.sh         image chroot: deploy firmware and ioctl_app
set -e
cd "$(dirname "$0")"
if [ -n "$1" ]; then
    mkdir -p "$1"
    cp -a scripts firmware tools install.sh README.md "$1/"
else
    install -m 644 firmware/*.bin /lib/firmware/
    install -m 755 tools/ioctl_app /usr/bin/ioctl_app
fi
