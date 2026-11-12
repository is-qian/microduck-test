# vqe-test — facemic PDM mic VQE (ANR+AGC) processing chain

A runnable wrapper of the SDK (`librkaudio_vqe`) on the microduck board
(RK3566). The link has no hardware gain (see `../facemic-gain/`); VQE
provides an ANR denoise + AGC dynamic voice front end, suitable for
feeding ASR/intercom; pick either this or the fixed softvol gain. VQE
input must be the raw stream (`-D hw:facemic,0`) — do not stack softvol
on top.

Only sources are committed (Rust, zero external crates); `install.sh
<dir>` is called by `../build-dist.sh`/CI, cross-builds and packs the
self-contained payload (bin + so + config + wrapper) into the dist — on
the board it lives at `/opt/microduck-test/vqe-test/`, ready after
unpacking. The closed-source library is vendored in `vendor/` (same
approach as `nfc/rfal`), so CI needs no SDK; to change library versions,
point `SDK=` at a full SDK build.

Build note: unlike hwtest's musl static build, this tool uses
`aarch64-unknown-linux-gnu` dynamic linking — the closed-source
`librkaudio_vqe.so` is a glibc build that a musl static binary cannot
load. The highest glibc symbol requirement produced by Rust measured
2.34 (board has 2.36, compatible).

## Known Pitfalls (verification conclusions, 2026-10-29)

- `librkaudio_common.so` reads `/proc/device-tree/compatible` against a
  chip whitelist that only accepts `rockchip,rv1126b` — rk3566 is
  rejected. The wrapper fakes it transiently with `mount --bind`,
  restored automatically on exit; before productionizing, confirm a
  library version supporting rk3566 with RK/Seeed
- **Must go through `rkaudio_preprocess_init_by_conf(json)`**: calling
  `init()` with a hand-written `RKAUDIOParam` silently outputs all zeros
  on this library build (the program keeps the numeric flags parameter
  for side-by-side debugging)
- Frame parameters: 256 samples/frame @16k (16ms), 2ch interleaved; the
  4th argument of `preprocess_short` is 512 (shorts); the SDK asr demo
  sources pass bytes, contradicting the official PDF docs — trust the
  docs. The return value is a byte count (512 bytes per frame = 256
  shorts, measured on board)
- Input requirement: 16k mono S16 wav; the program duplicates it to 2ch
  internally before feeding the library

## On-Board Usage (/opt/microduck-test/vqe-test/)

### Live (recommended)

```sh
sudo /opt/microduck-test/vqe-test/vqe-live /tmp/raw.wav /tmp/out.wav [seconds, default 10]
```

Captures the facemic raw stream (hw:facemic, no softvol) → VQE frame by
frame → writes two mono S16 files (raw reference + processed), printing
live in/out levels every second. When integrating a voice service, the
stdin loop in `vqe_live.c` is the reference template (the SDK's
`app/lvgl_demo/rk_demo/asr/audio.c` is the full version with wake word).

### Offline

```sh
# Record the raw stream (S32 2ch) and convert to 16k mono S16:
sudo arecord -D hw:facemic,0 -f S32_LE -r 16000 -c 2 -d 8 /tmp/r.wav
python3 -c "
import wave, array
w = wave.open('/tmp/r.wav'); a = array.array('i'); a.frombytes(w.readframes(w.getnframes()))
m = a[0::2]
o = array.array('h', [max(-32768, min(32767, x >> 16)) for x in m])
wo = wave.open('/tmp/in.wav', 'wb'); wo.setnchannels(1); wo.setsampwidth(2); wo.setframerate(16000)
wo.writeframes(o.tobytes()); wo.close()"

sudo /opt/microduck-test/vqe-test/vqe-test /tmp/in.wav /tmp/out.wav
```

Expected: pure-noise segments drop in output rms (ANR), speech/transient
segments rise (AGC). Reference (ambient sound): rms 82.5 → 260.5
(+10dB); pure noise floor: 72.3 → 15.2 (−13.5dB).

## config_mic.json

Generated from the SDK's `rkaudio_algorithms/conf/config_aivqe.json`:
every module `status` set to disable, keeping only `bf`/`agc`/`anr`
enabled (default parameter templates). AEC/wakeup/AINR are not enabled
(the latter needs an NPU rknn model).
