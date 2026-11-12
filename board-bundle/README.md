# board-bundle

The robot's daemon set (robotd, configd, padd, nfcd, …) plus the official
policy set, preinstalled into FULL images so a fresh board comes up ready to
drive — no scp step, no network on the board.

## Everything here builds from source

Nothing prebuilt is committed. `build-daemon.sh` produces the payload:

1. checks out the pinned `pollen-robotics/microduck` rev and applies
   `patches/*.patch` (our delta on top of upstream)
2. cross-builds the daemons with `cargo zigbuild`, exactly like upstream CI
3. renders the voice bank from the pinned `VOICE_SEED` — one voice for
   every robot of an image (a real robot's voice derives from its SoC
   serial; that trade buys sound out of the box. A robot that should
   sound like itself reruns `sounds ensure-bank` on board and the
   `.seed` marker mismatch regenerates it)
4. packs the release tree with the upstream `xtask package` invocation and
   unpacks it as the preinstalled release
5. fetches ONNX Runtime and the policy set, pinned and sha256-checked
   (`ORT_SHA256` in the script, `policies.sha256` next to it)

Run it on a dev host with cargo, zig, cargo-zigbuild, zstd and curl:

```sh
./build-daemon.sh                 # clones from GitHub
MICRODUCK_SRC=../microduck ./build-daemon.sh   # reuse a local checkout
```

The dist CI runs the same script, so the image build never compiles anything.

## Layout

- `install.sh` — three modes, per the repo contract:
  - `install.sh <dir>` stages the payload into the dist tarball
  - `install.sh` (no args, image chroot) installs into the image and
    enables every unit that ships an `[Install]` section — the same rule
    the daemon's own `hooks/postinstall` uses
  - `install.sh --board` installs on a live board (`systemctl enable
    --now`); the rescue path for minimal images
- `patches/` — our delta on the pinned upstream rev (daemon v0.17.8,
  policy set v8). Today:
  `0001-configd-auto-reconnect.patch` (configd re-pages a bonded classic
  pad whose link dropped after pairing, and a background sweep reconnects
  bonded pads — e.g. right after an NFC-tag pairing),
  `0002-pad-presets.patch` (the roulade runs by default on the pad's
  software-Y button — the one labeled X on Nintendo-layout pads — and
  the bumpers kick again, restoring what upstream unbound in v0.17.8;
  set in code, so the shipped example config stays all-defaults and
  `pad reset y` also lands on the roulade)
  and `0003-roller-mode-support.patch` (roller mode on the pad, end to
  end: a binding named `mode:walk` / `mode:roller` / `mode:toggle`
  switches the locomotion mode instead of firing a skill —
  `robotctl pad bind y mode:toggle` flips walk<->roller on one button,
  never repeating while held; the status LED blinks green while healthy
  on wheels, so an accidental switch is visible; and the idle
  look-around is walk-mode only, because its head sweep is a balance
  disturbance the roller policy was never trained on — the duck drove
  fine and fell seconds after every stop; and the walking-trained
  one-shots stay off the wheels: in roller mode the roulade, the kicks
  and the sit toggle are refused, because every one of them tips a
  wheeled robot over — the crouch, trained for wheels, keeps the
  ground-pick trigger)
- `patches/0004-nfc-pairing-marker.patch` — an NFC tag
  holding the text `microduck_gamepad_pairing` pairs whichever pad is
  in pairing mode advertising the name after it (a Pro Controller when
  it names no other; `microduck_gamepad_pairing:8BitDo Pro 2` names
  one). The tag-triggered scan runs 60 s, outlasting the pad's own
  one-minute pairing window, so the tag can be touched before the
  pad's Sync button. A bonded pad of the same name does not end the
  scan — only a fresh, unbonded one does — so a second pad pairs by
  tag while the first stays bonded. Tags carrying a MAC address keep
  working unchanged. Threads a `name` through `pad.pair` (proto,
  configd, robotctl).
- `patches/0005-nfc-type-v.patch` — the reader also speaks ISO 15693,
  so an NFC Type 5 tag (ST25TV, ICODE) pairs a pad exactly as a Type 2
  one does: each poll asks ISO 14443A first, then a single-slot 15693
  inventory, on each antenna in turn. A Type 5 tag's memory is a
  capability container followed by the same TLV area a Type 2 tag
  carries, so the NDEF walk and the pairing chain are shared. The
  ST25R100 driver also gains ST's reader library's chip init — the AM
  regulator, modulation depth and receiver gain — which ISO 15693
  needs and the 14443A-only driver never did, plus its inventory flags
  byte 0x26, without which a tag does not answer. Write such a tag
  with `nfc-test t5 ndef`.
- `patches/0006-face-board-v1-led-control.patch` — face LED control on
  the V1.0 board: `Led::set(On)` wrote `brightness = 1`, which
  gpio-leds read as on but the V1.0 PWM drivers (AW21009/AW2023) dim
  by the value — On now writes `max_brightness`; and the LED daemons'
  units hand back only the V0 gpio-leds sysfs path from
  `ProtectKernelTunables`' read-only /sys, so every V1.0 write failed
  with EROFS — they now also hand back the I2C LED paths (AW21009 at
  3-0020, AW2023 at 3-0045). Verified on a V1.0 board: Home toggles
  the flashlight at full brightness.
- `udev/99-robot-spidev.rules` — spidev1.0/3.0 group access for nfcd/tofd;
  bundle-specific, not from upstream
- alsa state: moved to `test-tools/alsa-config` (mic softvol gain + speaker
  Playback Path SPK_HP in one asound.state, baked at image build)
- `robot-services` — `robot-services start|stop`, installed to
  `/usr/local/sbin`: brings the whole daemon set down (e.g. before running
  hwtest, which needs the camera, NFC reader and motor bus free) and back
  up. The set is read off the installed release, so a new daemon is
  picked up without editing the script
- `snapshot/`, `policies/` — build products, gitignored

## Pairing a gamepad from an NFC tag

`nfcd` (enabled by the install) polls the reader continuously, asking as
ISO 14443A and then as ISO 15693 on each antenna: a Type 2 tag (NTAG) and a
Type 5 one (ST25TV, ICODE) both work. A touched tag pairs whichever pad is
in pairing mode advertising the name it carries, or the one at the address
it carries. Write the tag on the board with the nfc tool (stop `nfcd`
first; it owns the reader):

```sh
sudo systemctl stop nfcd
sudo /opt/microduck-test/nfc/nfc-test t2 ndef microduck_gamepad_pairing   # Type 2 (NTAG)
sudo /opt/microduck-test/nfc/nfc-test t5 ndef microduck_gamepad_pairing   # Type 5 (ISO15693)
# a named pad:    t2/t5 ndef "microduck_gamepad_pairing:8BitDo Pro 2"
# by MAC address: t2/t5 ndef AA:BB:CC:11:00:03
sudo systemctl start nfcd
```

The tag holds one NDEF text record. The bare marker pairs a Pro
Controller; `microduck_gamepad_pairing:<name>` pairs a pad advertising
`<name>`; a MAC address (`:` or `-` separators, either case) pairs that
pad. The tag-triggered scan runs 60 s, so the tag can be touched before
the pad's Sync button. Pairing rules: a pad already connected wins, and
one touch is one attempt — lift and touch again to retry. The patched
configd re-pages the pad if the fresh link drops, so a pairing leaves
the pad connected, and its background sweep reconnects bonded pads that
disappear later.

Write tags with `nfc-test`, not a phone app: phone NFC apps write a
Lock Control TLV ahead of the record, which `nfcd` does not walk past —
such a tag anticollides but never reads (receiver CRC errors on the
FAST_READ fallback), looking like a dead tag until it is rewritten on
the board.

## Upgrading the daemon

Bump `REV`/`VERSION` in `build-daemon.sh` (and `VOICE_SEED` if the voice
should change), refresh patches if upstream moved, re-run it, and test
on a board before cutting a dist release.
