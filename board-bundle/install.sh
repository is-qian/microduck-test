#!/bin/sh
# Daemon + policy set, preinstalled into FULL images.
#
# Modes (see ../build-dist.sh and ../microduck-test-tools.sh):
#   install.sh <dir>    dist build: stage the payload into <dir>
#   install.sh          image chroot: install into the image (no systemd running)
#   install.sh --board  live board: same, plus enable --now (minimal-image rescue)
#
# snapshot/ and policies/ are build products of ./build-daemon.sh; dist mode
# builds them on demand, the other modes expect them present.
set -eu
cd "$(dirname "$0")"

if [ -n "${1:-}" ] && [ "$1" != "--board" ]; then
    # dist mode: the payload ships as-is; the chroot copy of this script does
    # the install inside the image. Build it first when missing (CI runs
    # straight from a clean checkout).
    [ -d snapshot ] && [ -d policies ] || bash ./build-daemon.sh
    mkdir -p "$1"
    cp -a snapshot policies install.sh robot-services "$1/"
    exit 0
fi

ON_BOARD=0
[ "${1:-}" = "--board" ] && ON_BOARD=1

SNAP=snapshot
REL=$(basename snapshot/opt/robot/daemon/releases/*)
INSTALL_DIR=/opt/robot/daemon
POLICY_ROOT=/opt/robot/policies

[ -d "$SNAP/opt/robot/daemon/releases/$REL" ] || { echo "payload missing: run ./build-daemon.sh first" >&2; exit 1; }

say() { echo "==> $*"; }

# A live board is running the very binaries being replaced; a running
# executable cannot be overwritten (ETXTBSY).
if [ "$ON_BOARD" = 1 ]; then
    systemctl stop robotd configd updaterd btd padd mediad tofd nfcd 2>/dev/null || true
fi

say "daemon release $REL"
mkdir -p "$INSTALL_DIR/releases"
rm -rf "$INSTALL_DIR/releases/$REL"
cp -a "$SNAP/opt/robot/daemon/releases/$REL" "$INSTALL_DIR/releases/"
ln -sfn "releases/$REL" "$INSTALL_DIR/current"

say "ONNX Runtime"
install -m 0644 "$SNAP"/usr/local/lib/libonnxruntime.so.* /usr/local/lib/
ort=$(basename "$SNAP"/usr/local/lib/libonnxruntime.so.*)
ln -sf "$ort" /usr/local/lib/libonnxruntime.so.1
ln -sf "$ort" /usr/local/lib/libonnxruntime.so
ldconfig

say "config (/etc/robot) — an existing robotd.toml is kept"
mkdir -p /etc/robot/trusted_keys
install -m 0644 "$SNAP/etc/robot/updater.toml" /etc/robot/updater.toml
install -m 0644 "$SNAP"/etc/robot/trusted_keys/*.pub /etc/robot/trusted_keys/
if [ -f /etc/robot/robotd.toml ]; then
    echo "    robotd.toml exists, not overwriting (check board.version and bus.port yourself)"
else
    install -m 0644 "$SNAP/etc/robot/robotd.toml" /etc/robot/robotd.toml
fi

say "users and groups"
mkdir -p /usr/lib/sysusers.d
for f in "$INSTALL_DIR"/current/systemd/sysusers.d/*.conf; do
    install -m 644 "$f" /usr/lib/sysusers.d/
done
systemd-sysusers 2>/dev/null || true
for d in btd padd; do
    getent passwd "$d" >/dev/null || \
        useradd --system --no-create-home --shell /usr/sbin/nologin "$d" || true
done
getent group robot >/dev/null || groupadd --system robot
for u in seeed "${SUDO_USER:-}"; do
    [ -n "$u" ] && id "$u" >/dev/null 2>&1 && usermod -aG robot "$u" || true
done

say "systemd units"
install -m 644 "$SNAP"/etc/systemd/system/*.service "$SNAP"/etc/systemd/system/*.timer /etc/systemd/system/
src="$INSTALL_DIR/current/deploy/journald.conf.d/10-robot.conf"
if [ -f "$src" ]; then
    mkdir -p /etc/systemd/journald.conf.d
    install -m 644 "$src" /etc/systemd/journald.conf.d/
fi
ln -sfn "$INSTALL_DIR/current/bin/robotctl" /usr/local/bin/robotctl
mkdir -p /usr/local/sbin
for s in robot-rescue robot-boot-check; do
    [ -f "$INSTALL_DIR/current/scripts/$s" ] && install -m 755 "$INSTALL_DIR/current/scripts/$s" /usr/local/sbin/ || true
done
install -m 755 robot-services /usr/local/sbin/
[ -f "$INSTALL_DIR/current/scripts/setup-login.sh" ] && sh "$INSTALL_DIR/current/scripts/setup-login.sh" || true
[ -f "$INSTALL_DIR/current/scripts/setup-quiet-boot.sh" ] && sh "$INSTALL_DIR/current/scripts/setup-quiet-boot.sh" || true

say "udev rule (spidev access for nfcd/tofd)"
install -m 0644 "$SNAP/etc/udev/rules.d/99-robot-spidev.rules" /etc/udev/rules.d/

say "policy set"
pver=$(cat policies/.version 2>/dev/null || echo local)
target="$POLICY_ROOT/releases/seed-$pver"
mkdir -p "$target"
install -m 0644 policies/*.onnx policies/manifest.json "$target/"
if [ ! -f "$target/.source" ]; then
    { echo "repo=pollen-robotics/microduck-policies"; echo "version=$pver"; \
      echo "fetched=$(date -u +%Y-%m-%dT%H:%M:%SZ)"; } > "$target/.source"
fi
ln -sfn "releases/seed-$pver" "$POLICY_ROOT/current"
mkdir -p /var/lib/robot/policies /var/lib/robot/updater
chgrp -R robot /var/lib/robot /opt/robot/policies 2>/dev/null || true

say "voice bank (rendered at dist build, one voice per image)"
if [ -d "$SNAP/var/lib/robot/sounds" ]; then
    install -d /var/lib/robot
    cp -a "$SNAP/var/lib/robot/sounds" /var/lib/robot/
    chgrp -R robot /var/lib/robot/sounds 2>/dev/null || true
else
    echo "    no bank in the snapshot; the robot stays quiet" >&2
fi

say "enabling units"
if [ "$ON_BOARD" = 1 ]; then
    systemctl daemon-reload
    udevadm control --reload
fi
# One rule, read off the unit files like hooks/postinstall does: every unit
# with an [Install] section gets enabled; nothing else does.
for unit in "$INSTALL_DIR"/current/systemd/*.service "$INSTALL_DIR"/current/systemd/*.timer; do
    [ -f "$unit" ] || continue
    wanted=$(sed -n 's/^WantedBy=//p' "$unit" | head -1)
    [ -n "$wanted" ] || continue
    name=$(basename "$unit")
    if [ "$ON_BOARD" = 1 ]; then
        systemctl enable --now "$name"
    else
        mkdir -p "/etc/systemd/system/$wanted.wants"
        ln -sfn "/etc/systemd/system/$name" "/etc/systemd/system/$wanted.wants/$name"
    fi
done

echo
say "done. Check: robotctl health"

# The image keeps no second copy of the payload: everything above installed
# it into its final location. Only the chroot copy self-destructs; a
# --board run stays put for re-runs.
if [ "$ON_BOARD" = 0 ]; then
    cd /
    rm -rf "$(dirname "$0")"
fi
