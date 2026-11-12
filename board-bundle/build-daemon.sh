#!/bin/bash
# Build the daemon payload from source.
#
#   1. check out the pinned upstream microduck rev, apply patches/*.patch
#   2. cross-build every shipped daemon binary (cargo zigbuild, like upstream CI)
#   3. assemble the release tree exactly the way `xtask package` packs it
#   4. fetch ONNX Runtime and the official policy set, pinned and sha256-checked
#
# Output lands in snapshot/ and policies/ next to this script (both gitignored;
# nothing prebuilt is committed). Re-run after bumping the pins below.
#
# Needs: cargo, zig + cargo-zigbuild, zstd, curl, git. MICRODUCK_SRC points at a
# local microduck checkout to skip the clone.
set -euo pipefail
cd "$(dirname "$0")"

# ── pins ─────────────────────────────────────────────────────────────────────
REV="942e58e3aa6dbb28e7037203fff11866122c7567"   # daemon v0.17.8
VERSION="0.17.8"
VOICE_SEED="2273011140"   # the voice every robot of an image shares; auditioned on the bring-up board (pick another: cargo run -p sounds -- hear --seed N)
ORT_VERSION="1.28.0"
ORT_SHA256="e15ff8b5d85afe6c144d97c6fd432254bf76a219daaf17658087d6ecb3e8f0bb"
POLICY_REPO="pollen-robotics/microduck-policies"
POLICY_VERSION="v8"
POLICY_FILES="alpha_ground_pick.onnx alpha_sitstand.onnx alpha_stand.onnx \
alpha_walking.onnx ball_kick_left.onnx ball_kick_right.onnx manifest.json \
roller.onnx roller_crouch.onnx roulade.onnx velstand.onnx"

BINARIES="updaterd robotctl robotd configd btd padd mediad sounds pet-detect pet-features tofd nfcd"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# ── 1. source ────────────────────────────────────────────────────────────────
echo "==> source: microduck @ ${REV:0:7}"
src="$work/microduck"
if [ -z "${MICRODUCK_REPO:-}" ]; then
    # CI runners can't reach github.com; clone the GitLab mirror with the job token.
    if [ -n "${CI_JOB_TOKEN:-}" ]; then
        MICRODUCK_REPO="http://gitlab-ci-token:${CI_JOB_TOKEN}@gitlab.internal.example/microduck/microduck.git"
    else
        MICRODUCK_REPO="https://github.com/pollen-robotics/microduck"
    fi
fi
if [ -n "${MICRODUCK_SRC:-}" ]; then
    git -C "$MICRODUCK_SRC" worktree list --porcelain | grep -q . || true
    git clone -q --no-checkout "$MICRODUCK_SRC" "$src"
else
    # The runner's outbound link is flaky; retry the clone.
    for attempt in 1 2 3 4 5; do
        git clone -q --no-checkout "$MICRODUCK_REPO" "$src" && break
        [ "$attempt" = 5 ] && exit 1
        sleep 10
    done
fi
git -C "$src" checkout -q "$REV"
for p in patches/*.patch; do
    [ -e "$p" ] || continue
    echo "    apply $p"
    git -C "$src" apply "$(pwd)/$p"
done

# ── 2. build ─────────────────────────────────────────────────────────────────
echo "==> aarch64 sysroot (target gstreamer + libudev, from the robot's Debian)"
export DUCK_SYSROOT="${DUCK_SYSROOT:-$work/sysroot}"
eval "$(sh "$src/scripts/cross-sysroot.sh" | grep '^export')"

echo "==> cargo zigbuild (this takes a while on a cold cache)"
( cd "$src" && DUCK_REVISION="$REV" DUCK_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.31 --bins )

# ── 3. assemble the release tree ─────────────────────────────────────────────
echo "==> assemble releases/$VERSION"
rel="snapshot/opt/robot/daemon/releases/$VERSION"
rm -rf snapshot/opt
mkdir -p "$rel/bin" "$work/staged"
for b in $BINARIES; do
    cp "$src/target/aarch64-unknown-linux-gnu/release/$b" "$work/staged/"
done
# `xtask package` packs the tar a robot receives over OTA; the preinstalled
# release is that same tree unpacked, so pack and unpack rather than
# hand-assembling a second layout that could drift.
( cd "$src" && cargo run -q -p xtask -- package \
    --version "$VERSION" --channel daemon \
    --bin-dir "$work/staged" --out "$work/dist" \
    --base-url "https://invalid.local/board-bundle" \
    --revision "$REV" \
    --allow-version-drift \
    --include "updater/systemd/updaterd.service=systemd/updaterd.service" \
    --include "updater/systemd/sysusers.d/robot.conf=systemd/sysusers.d/robot.conf" \
    --include "robotd/systemd/robotd.service=systemd/robotd.service" \
    --include "hooks/postinstall=hooks/postinstall" \
    --include "scripts/setup-gstreamer.sh=scripts/setup-gstreamer.sh" \
    --include "scripts/setup-npu.sh=scripts/setup-npu.sh" \
    --include "deploy/overlays/rk3568-npu-enable.dts=deploy/overlays/rk3568-npu-enable.dts" \
    --include "scripts/setup-rkaiq.sh=scripts/setup-rkaiq.sh" \
    --include "scripts/rkaiq-modinfo-shim.c=scripts/rkaiq-modinfo-shim.c" \
    --include "scripts/setup-login.sh=scripts/setup-login.sh" \
    --include "scripts/setup-quiet-boot.sh=scripts/setup-quiet-boot.sh" \
    --include "scripts/seed-policies.sh=scripts/seed-policies.sh" \
    --include "scripts/seed-detector.sh=scripts/seed-detector.sh" \
    --include "scripts/robot-rescue=scripts/robot-rescue" \
    --include "scripts/robot-boot-check=scripts/robot-boot-check" \
    --include "updater/systemd/robot-boot-check.service=systemd/robot-boot-check.service" \
    --include "updater/systemd/robot-boot-check.timer=systemd/robot-boot-check.timer" \
    --include "configd/systemd/configd.service=systemd/configd.service" \
    --include "btd/systemd/btd.service=systemd/btd.service" \
    --include "btd/systemd/sysusers.d/btd.conf=systemd/sysusers.d/btd.conf" \
    --include "padd/systemd/padd.service=systemd/padd.service" \
    --include "padd/systemd/sysusers.d/padd.conf=systemd/sysusers.d/padd.conf" \
    --include "mediad/systemd/mediad.service=systemd/mediad.service" \
    --include "mediad/systemd/sysusers.d/mediad.conf=systemd/sysusers.d/mediad.conf" \
    --include "tof/systemd/tofd.service=systemd/tofd.service" \
    --include "tof/systemd/sysusers.d/tofd.conf=systemd/sysusers.d/tofd.conf" \
    --include "nfc/systemd/nfcd.service=systemd/nfcd.service" \
    --include "nfc/systemd/sysusers.d/nfcd.conf=systemd/sysusers.d/nfcd.conf" \
    --include "deploy/journald.conf.d/10-robot.conf=deploy/journald.conf.d/10-robot.conf" \
    --include "docs/design/architecture.md=docs/architecture.md" \
    --include "docs/design/updater-design.md=docs/updater-design.md" \
    --include "deploy/README.md=docs/deploy.md" \
    --include "pet-detect/models/pet_detect.onnx=models/pet_detect.onnx" \
    --include "duck-control/models/pickup_detector.onnx=models/pickup_detector.onnx" )

artifact="$work/dist/daemon-$VERSION.tar.zst"
tar -C "$rel" --zstd -xf "$artifact"

# Provenance updaterd shows for this preinstalled release. The URL pair is
# where the OTA equivalent would live; a local build has none, and the field
# is display-only for an already-installed release.
size=$(stat -c%s "$artifact")
sha=$(sha256sum "$artifact" | cut -d' ' -f1)
cat > "$rel/.updater-manifest.json" << JSON
{
  "channel": "daemon",
  "min_hw_rev": 1,
  "schema_version": 1,
  "sha256": "$sha",
  "sig_url": "",
  "size": $size,
  "source_revision": "$REV",
  "url": "board-bundle local build",
  "version": "$VERSION"
}
JSON

# The voice the image ships with: rendered from the pinned seed, not the SoC
# serial a real robot derives its own voice from — one voice for every robot,
# in exchange for sound out of the box. A robot that should sound like itself
# reruns `sounds ensure-bank` on board (the .seed marker mismatch regenerates).
echo "==> voice bank (seed $VOICE_SEED)"
( cd "$src" && cargo build -q --release --bin sounds )
mkdir -p snapshot/var/lib/robot
"$src/target/release/sounds" ensure-bank --seed "$VOICE_SEED" \
    --dir snapshot/var/lib/robot/sounds

# Board-level files the release tree does not carry.
echo "==> board config (/etc/robot, units, udev)"
mkdir -p snapshot/etc/robot/trusted_keys snapshot/etc/systemd/system \
         snapshot/etc/udev/rules.d snapshot/usr/local/lib snapshot/var/lib/alsa
sed -e 's/^# version = "zero3"/version = "beta"/' -e 's|/dev/ttyS2|/dev/ttyS3|' \
    "$src/deploy/robotd.toml" > snapshot/etc/robot/robotd.toml
sed -e 's/^allow_dev_keys        = false/allow_dev_keys        = true/' \
    -e 's|"ORG/duck-daemon"|"pollen-robotics/microduck"|' \
    "$src/deploy/updater.toml" > snapshot/etc/robot/updater.toml
cp "$src"/deploy/trusted_keys/*.pub "$src"/deploy/dev-key/team.dev.pub \
   snapshot/etc/robot/trusted_keys/
cp "$rel"/systemd/*.service "$rel"/systemd/*.timer snapshot/etc/systemd/system/
cp udev/99-robot-spidev.rules snapshot/etc/udev/rules.d/
# alsa state moved to test-tools/alsa-config (mic gain + speaker path, one
# file, no overwrite race between bundles)

# ── 4. ONNX Runtime ──────────────────────────────────────────────
echo "==> ONNX Runtime $ORT_VERSION"
ort="$work/ort.tgz"
curl -fsSL --http1.1 --retry 5 --retry-all-errors -o "$ort" "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-linux-aarch64-$ORT_VERSION.tgz"
if [ -z "$ORT_SHA256" ]; then
    echo "    first fetch: sha256 is $(sha256sum "$ort" | cut -d' ' -f1)"
    echo "    pin it as ORT_SHA256 in this script and re-run"
    exit 1
fi
echo "$ORT_SHA256  $ort" | sha256sum -c -
tar -C "$work" -xzf "$ort"
install -m 0644 "$work/onnxruntime-linux-aarch64-$ORT_VERSION/lib/libonnxruntime.so.$ORT_VERSION" \
    snapshot/usr/local/lib/

# ── 5. policy set ────────────────────────────────────────────────
echo "==> policy set $POLICY_VERSION"
mkdir -p policies
# Probe once: the CI runner cannot reach huggingface.co, hf-mirror.com can
# serve the same files. sha256 is checked below regardless of the source.
hf_base=""
for host in huggingface.co hf-mirror.com; do
    if curl -fsSL --connect-timeout 20 -o /dev/null \
        "https://$host/$POLICY_REPO/resolve/$POLICY_VERSION/manifest.json"; then
        hf_base="https://$host/$POLICY_REPO/resolve/$POLICY_VERSION"; break
    fi
done
[ -n "$hf_base" ] || { echo "no reachable mirror for the policy set" >&2; exit 1; }
echo "    from $hf_base"
for f in $POLICY_FILES; do
    [ -f "policies/$f" ] || curl -fsSL --retry 3 --retry-all-errors -o "policies/$f" "$hf_base/$f"
done
echo "$POLICY_VERSION" > policies/.version
( cd policies && sha256sum -c ../policies.sha256 )

echo "==> done: snapshot/ and policies/ are ready for install.sh"
