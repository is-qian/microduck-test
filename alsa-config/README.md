# alsa-config — ALSA factory configuration (facemic mic gain + speaker path)

Maintains the whole-unit ALSA factory state in one place, applied once
at image build time:

- **Microphone**: the face-mic link (card "facemic", MSM261DHP006 PDM
  digital mic) has no hardware gain (the mic has no registers and the
  RK3566 PDM controller has no gain bits); the softvol `pcm.mic` from
  `/etc/asound.conf` provides a fixed +20dB (step 600-scale, 0.1dB/step,
  +20dB = step 400)
- **Speaker**: rk809 Playback Path **SPK_HP** — the only path that
  powers both the HPL DAC and the external NS4150B amplifier (the dts
  has no use-ext-amplifier; the SPK path powers the HPL DAC down),
  otherwise the unit ships silent at boot. This item moved here from
  board-bundle (2026-10-29), eliminating the whole-file asound.state
  overwrite race when the two install in sequence

## Files

- `asound.conf` — softvol device definition (record with `-D mic`)
- `asound.state` — the complete factory state (speaker Playback
  Path/DAC + Face Mic Capture Volume 400), organized by card name
  (state.facemic etc.), independent of card numbers

## Deployment

**Image build stage (automatic, zero on-board steps)**: the dist
`install.sh` runs once in the image chroot and writes the two files
above. At boot, udev alsa-restore rebuilds the softvol control per the
user markers in the state file and restores +20dB and SPK_HP — effective
from first boot (verified on a simulated first boot 2026-10-29: control
reads 400,400 after boot; the first recording already carries the gain).

**On-board repair (manual, idempotent)**: restore after the state is
lost or overwritten (same device stage as install.sh, effective
immediately):

```sh
sudo /opt/microduck-test/alsa-config/install.sh
```

## Usage

- Record with gain: `arecord -D mic ...` (opening `-D hw:facemic,0`
  directly bypasses the gain)
- Verify: `amixer -c facemic cget name='Face Mic Capture Volume'`
  should read 400,400
- Adjust gain: +1dB per 10 steps (max 599 = +39.9dB); above +24dB
  (step 440) loud close-range speech may clip; after changing run
  `sudo /usr/sbin/alsactl store`
- Recording must use `-c 2` (the PDM controller needs at least 2 channels)
- Note: alsactl lives in `/usr/sbin`; non-interactive ssh shells need
  the full path
