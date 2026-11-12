#!/bin/sh
# Two modes (see ../build-dist.sh and ../microduck-test-tools.sh):
#   install.sh <dir>   dist build: stage the .deb payload into <dir>
#   install.sh         image chroot: install the packages
set -e
cd "$(dirname "$0")"
if [ -n "$1" ]; then
    mkdir -p "$1"
    cp -a ./*.deb install.sh "$1/"
else
    dpkg -i ./*.deb
fi
