# MicroDuck test tools

Factory/bring-up test tools preinstalled into FULL (non-minimal) images.
Minimal images never ship this repo.

## Tools

| Tool | Purpose |
|---|---|
| `alsa-config/` | Factory ALSA state baked into the image: mic softvol +20dB (`pcm.mic`) and speaker Playback Path SPK_HP, applied by boot-time alsa-restore |
| `dxl/` | Dynamixel 2.0 bus tool for the D4 bus (GD32 power board + XL330 servos) |
| `hwtest/` | Menu-driven hardware test, mirrors `test/hardware-test.md`; mic items record via `-D mic` (needs `alsa-config`) |
| `nfc/` | ST25R100 NFC poll / ESL e-ink tag write & refresh |
| `packages/` | Extra .deb packages installed into the image |
| `re-test/` | Full-function run-in (aging) test, all peripherals in parallel |
| `rockchip-test/` | RK3566 SDK test suite (CPU/DDR stress, peripherals) |
| `tof/` | VL53L8CX SPI ranging test for the Face board |
| `vqe-test/` | Rockchip VQE (ANR + AGC) for the face mic: offline file processing and realtime capture, on the closed-source SDK libs |
| `wifi-rf-test/` | WQ9201 WiFi/BT fixed-frequency (RSE) certification test |

## CI dist

`Build > Run pipeline` cross-builds every tool for aarch64 and publishes
`test-tools-dist.tar.gz` as the latest release. The tarball holds one
directory per tool, laid out as it lands under `/opt/microduck-test/`,
plus the Armbian extension entry point `microduck-test-tools.sh`.

The armbian-build CI deploys the latest release into FULL images instead
of cloning this repo: the extension copies each tool directory into the
image and `dpkg -i` the debs. Nothing compiles in the image chroot.

## Adding a tool

Add a directory that follows the dist contract; neither `build-dist.sh`
nor the Armbian extension need changes:

| Path | Meaning |
|---|---|
| `<tool>/install.sh <dir>` | dist-build entry: compile (via `./build.sh` when needed) and install the runtime payload into `<dir>` |
| `<tool>/install.sh` | same script, no args: runs once in the image chroot for deploy-only extras (e.g. firmware installs); only tools that ship their install.sh in the payload get this |
| `<tool>/` without `install.sh` | the whole directory ships as-is (script-only tools) |

## How it gets into the image

The FULL-image CI job (`FULL=true`) downloads the dist release into
`armbian-build/userpatches/test-tools`, symlinks it as
`userpatches/extensions/microduck-test-tools`, and enables the
`microduck-test-tools` extension. `TEST_TOOLS_REF` pins the release tag
(`latest` by default).

For a local FULL build, stage a dist the same way:

```sh
./build-dist.sh   # in this repo, needs rust aarch64-musl + aarch64 gcc
cd armbian-build
tar -xzf <this-repo>/test-tools-dist.tar.gz -C userpatches/test-tools
mkdir -p userpatches/extensions
ln -sfn ../test-tools userpatches/extensions/microduck-test-tools
# then build with BUILD_MINIMAL=no and:
#   enable_extension "microduck-test-tools"
# in your userpatches config
```

## Provenance

- `rockchip-test/`: RK3566 SDK `external/rockchip-test`, see its LICENSE
- `alsa-config/asound.state`: factory snapshot taken from a verified board
  (speaker Playback Path SPK_HP + Face Mic softvol 400), keyed by card
  name so it does not depend on card indices
- `vqe-test/`: `librkaudio_vqe.so` and `librkaudio_common.so` are
  Rockchip binary blobs from the RK3566/RK3568 Linux SDK
  (`external/common_algorithm/audio/rkaudio_algorithms`, VQE build
  git-dcd71af 2025-10-15); the lib's SoC whitelist only accepts
  `rockchip,rv1126b`, so the wrappers bind-mount a faked compatible
  around each run (see `vqe-test/README.md` for the full caveats)
- `packages/stressapptest_1.0.9-3_arm64.deb`: snapshot.debian.org (dropped from
  bookworm; sid builds need the t64 libaio)
- `tof/`: ST VL53L8CX ULD (STSW-IMG040 via stm32duino/VL53L8CX)
  with a Linux spidev platform layer
- `nfc/`: ST25R100 tools on a vendored ST RFAL (`rfal/`, see
  `nfc/LICENSE.st25rx00`) with a Linux spidev platform layer;
  `nfc/stb_image.h` is public domain (nothings/stb)
- `wifi-rf-test/`: WuQi WQ9201 MP factory-test firmware
  (`firmware/*.bin`) and `tools/ioctl_app` are vendor-provided binaries;
  `reference/wq-commands.txt` is the vendor command cheat sheet
