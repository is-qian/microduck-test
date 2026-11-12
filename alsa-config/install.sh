#!/bin/sh
# test-tools dist contract, two phases in one file:
#   host build:  install.sh <dir>   stage payload into <dir> (this file ships)
#   device:      install.sh         bake the factory ALSA config into the
#                                  image chroot / board (root, idempotent)
set -e
cd "$(dirname "$0")"

if [ -n "$1" ]; then
	mkdir -p "$1"
	install -m 644 asound.conf asound.state README.md "$1/"
	install -m 755 install.sh "$1/install.sh"
	exit 0
fi

[ "$(id -u)" = 0 ] || { echo "run as root: sudo ./install.sh"; exit 1; }

install -m 0644 asound.conf /etc/asound.conf
install -m 0644 asound.state /var/lib/alsa/asound.state
# apply now when hardware is present (no-op/fails softly inside a chroot);
# otherwise first boot's alsa-restore applies it
/usr/sbin/alsactl restore || true
