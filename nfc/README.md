# nfc-test

ST25R100 bring-up / production test tool. Uses a vendored ST RFAL
(`rfal/`, polled-register variant for boards without a working IRQ line)
with a Linux spidev platform layer.

Built by `install.sh` during image assembly; installed to
`/opt/microduck-test/nfc/`.

## Commands

| Command | Purpose |
|---|---|
| `poll [ms]` | Discover NFC-A / NFC-B / NFC-V tags, print UID + type |
| `apdu HEX...` | Raw ISO-DEP APDU exchange (Type 4 tags) |
| `nfcv read <blk>` | Read one 4-byte ISO15693 block |
| `nfcv write <blk> [hex]` | Write block (no data = write back what was read); verifies readback |
| `field` | Continuous amplitude/IQ readout for antenna coupling checks |
| `carrier` | Unmodulated 13.56 MHz CW carrier on (RSE/EMC), Ctrl-C to stop |
| `regs` | Dump key ST25R100 registers (field on) |
| `t2 read <page>` | Read four Type 2 (NTAG) pages, 16 bytes from `page` |
| `t2 write <page> <hex8>` | Write one 4-byte page; verifies readback |
| `t2 ndef <text>` | Write a one-record NDEF text message at page 4 |
| `t5 read` | Dump a Type 5 tag's container and TLV area, decode its NDEF text record |
| `t5 ndef <text>` | Write the same NDEF message as a Type 5 (ISO15693) tag: keeps a valid CC (writes one on a blank tag), then the TLV area from byte 4 |

`t2 ndef` / `t5 ndef` write the format `nfcd` reads when it pairs a gamepad
from a tag: a text record carrying the pad's Bluetooth address or the
pairing marker, e.g. `t2 ndef AA:BB:CC:11:00:03`.

`nfcd` owns the reader while it runs, so stop it for the write and start
it again after:

```sh
sudo systemctl stop nfcd
sudo /opt/microduck-test/nfc/nfc-test t2 ndef AA:BB:CC:11:00:03
# a Type 5 (ISO15693) tag instead:
sudo /opt/microduck-test/nfc/nfc-test t5 ndef microduck_gamepad_pairing
sudo systemctl start nfcd
```

Antenna selection: default is differential (both RFOx). `NFC_ANT=1`
forces single-ended RFO1 (J30), `NFC_ANT=2` forces RFO2 (J31). Applies
to all commands.

```sh
sudo /opt/microduck-test/nfc/nfc-test poll 8000
sudo NFC_ANT=2 /opt/microduck-test/nfc/nfc-test nfcv read 0
```

## RSE / EMC Testing

For chamber measurements keep the NFC side transmitting continuously —
two modes:

```sh
# 1. Pure carrier (unmodulated 13.56 MHz CW) — carrier/spurious baseline
sudo /opt/microduck-test/nfc/nfc-test carrier

# 2. Modulated transmit (worst-case normal operation) — continuous polling emitting REQA/REQB/INVENTORY
sudo /opt/microduck-test/nfc/nfc-test poll 3600000
```

- During this the lab scans with a magnetic-field loop antenna
  (9 kHz–30 MHz, 13.56 MHz fundamental and near spurious) and an
  electric-field antenna (above 30 MHz) separately
- Test both antenna interfaces: default differential (both RFOs);
  `NFC_ANT=1`/`NFC_ANT=2` force single-ended RFO1/RFO2
- Finish with Ctrl-C and a power cycle/reboot; no driver state is left
  behind

## ESL e-ink tag (FMSC_E701_CA, 1.54" six-color 240x240)

The `write` / `refresh` / `busywait` / `esl-info` commands drive the
Fudan Microelectronics ESL tag over ISO-DEP, following the vendor
Android SDK command flow.

```sh
sudo NFC_ANT=2 /opt/microduck-test/nfc/nfc-test esl-info
sudo NFC_ANT=2 /opt/microduck-test/nfc/nfc-test write image.png
sudo NFC_ANT=2 /opt/microduck-test/nfc/nfc-test write          # built-in 6-color bar pattern
```

- Image must be exactly 240x240; PNG/JPG/BMP/PPM are decoded via the
  bundled `stb_image.h` (public domain). With no image argument a
  built-in test pattern (six vertical color bars + black frame) is
  flashed instead.
- Pixels are quantized to the six panel colors with Atkinson
  dithering by default (the vendor iOS app's default mode). Plain
  nearest-color ("posterize" mode) via `NFC_DITHER=none`, Android-SDK
  Floyd-Steinberg via `NFC_DITHER=fs`.
- `write` prints the transfer rate per plane plus a total
  (`transfer total: 28800 bytes in ... = ... kB/s`); ~7.5 kB/s
  (60 kbps payload over the 106 kbps link) is normal for a
  well-coupled tag.
- `write` uploads all four bitplanes, issues the refresh command
  (`F0 D4`, P2 bit7 set = return immediately), then polls `F0 DE`
  until the panel reports done.
- During the waveform the tag brown-out resets and APDU exchanges may
  fail transiently; the tool retries these automatically. Keep the tag
  on the antenna until `refresh done` is printed.
- `refresh [P1hex]` re-runs only the display refresh for the image
  already in the tag; `busywait [secs]` polls `F0 DE` without starting
  a new refresh.

### Options (environment variables)

| Variable | Default | Effect |
|---|---|---|
| `NFC_ANT` | differential | `1` = single-ended RFO1 (J30), `2` = single-ended RFO2 (J31) |
| `NFC_DITHER` | `atkinson` | `none` = plain nearest-color (posterize), `fs` = Floyd-Steinberg (Android SDK) |
| `NFC_BLUR` | off | Box-blur radius before quantizing; use `1`-`2` for camera-captured photos (pixel-grid/moire noise otherwise washes colors out) |
| `NFC_D2_DELAY_MS` | off | Delay between image packets; lets the tag capacitor recharge. On any >150 ms packet the pace auto-doubles (up to 50 ms) |
| `NFC_PRECHARGE_MS` | off | Extra field-on delay before the image burst (helps when writing right after a refresh) |
| `NFC_REFRESH_P1` | `85` | Refresh power-on delay, P1 x 100 ms. `FF` (25.5 s) is recommended for dense/dark images (heavy waveform load) |

Energy notes: a packet taking ~10 s (`slow pkt: ... ok`) is the tag
stalling on an NVM write while its capacitor recharges; it completes
intact, just slowly. `NFC_D2_DELAY_MS=10` mostly avoids such stalls.
Dense/dark images maximize refresh current; if refresh wedges the tag
(stuck busy), recover it with the vendor phone app and retry with
`NFC_REFRESH_P1=FF`.

Relevant tag status words: `009000` done, `019000` busy, `68CA`
low-voltage during refresh, `6D00` instruction rejected while busy.

## nfc-id-test

Minimal SPI smoke test: reads the ST25R100 IC_ID register (expect
`0xA9`). No RFAL, single-file build:

```sh
gcc -O2 -o nfc-id-test nfc-id-test.c
```
