# MicroDuck Full-Feature Run-in (RE) Test

`re-test.sh` stresses all peripheral modules simultaneously on the board
for whole-unit reliability / burn-in testing.
With the FULL image build it is preinstalled to `/opt/microduck-test/re-test/`
alongside test-tools.
Each module runs as an independent background process with its own log;
modules whose hardware is absent are auto-skipped after a precheck and
logged.
The main loop records temperature / frequency / liveness every 10s and
prints a one-line key-status summary to the screen every 60s;
Ctrl-C, timeout or overheat (default 85°C) triggers unified cleanup and
restore.

Screen summary example:

```text
[14:19:21] STATUS 40C 1608MHz alive=9/9 | servo: 1:1863@30C 2:1854@29C bat:8.3V | fan:629Hz | wifi:94AP | nfc:nfcid=8E4BF51D | tof:14.76 fps
```

Fields in order: SoC temperature, CPU frequency, live process count,
per-servo position@temperature with battery voltage, fan RPM, number of
APs found by the WiFi scan, NFC tag UID (`no-tag` when absent), measured
ToF frame rate; a module dying prints `module xxx died` immediately.

Modules running concurrently:

| Module | Behavior |
|------|------|
| bt | Put the gamepad into pairing mode; it auto scans→pairs→trusts→connects and reconnects on link loss; a discovery scan every 60s (BT/WiFi coexistence stress); captures pad events when evtest is present |
| wifi | Triggers a full-channel scan every 10s, logs BSS count and connection state |
| cam | Continuous 1080p capture from `/dev/video0` (900 frames per cycle) to /dev/null |
| spk | Loops an 880Hz sine (RK809 → NS4150B) |
| mic | facemic PDM 2-channel 16kHz recording, MIC_REC seconds record / MIC_PAUSE seconds pause loop (default 5s/5s, discarded, not written to disk) |
| nfc | `nfc-test poll 5000` polling loop (tag must sit on the antenna) |
| tof | Auto-stops tofd → `vl53l8cx-test` 8×8 ranging, 300 frames (20s) per cycle, reports measured fps; restores tofd on exit |
| imu | LSM6DSV16X on the face board streaming at 120Hz, sampled at 1Hz, raw registers logged |
| led | All face LEDs on: AW2023 3× RGB + AW21009 9 channels (all off on exit) |
| fan | pwmfan at full speed, TACH frequency counted every 30s |
| servo | Auto-scans the bus (excluding GD32 200) → reads status → homes all servos (2048) → each servo slowly sweeps ±25° (speed limited by SERVO_VEL, default ≈1.8s/25°); every 10 half-cycles logs each servo's position/temperature/voltage/hardware errors + GD32 `dxl sensors` (IMU/battery) |

## Usage

Run on the board as root; prefer the USB gadget ssh or the serial
console (the continuous WiFi scan degrades WiFi ssh):

```sh
sudo /opt/microduck-test/re-test/re-test.sh        # preinstalled in the image
# manual deploy while developing: scp test-tools/re-test/re-test.{sh,conf} seeed@<board>:~/re-test/
```

All configurable parameters live in `re-test.conf` next to the script
(with comments) — edit the file directly (the preinstalled
`/opt/microduck-test/` is read-only by convention; copy to `~/re-test/`
and edit there — the script looks for `re-test.conf` next to itself and
in `$PWD`);
environment variables override the config file
(`VAR=x sudo -E ./re-test.sh` for a one-off):

| Parameter | Default | Description |
|------|------|------|
| `DURATION` | 0 | Test duration (seconds), 0 = run until Ctrl-C |
| `TEMP_MAX` | 85000 | Overheat protection threshold (millidegrees C) |
| `SKIP` | empty | Modules to disable, space-separated |
| `BT_MAC` | empty | Gamepad MAC; empty disables the bt module |
| `SERVO_IDS` | auto | Servo ID list, auto = bus scan (excluding GD32 200) |
| `SERVO_BAUD` | 1000000 | Bus baud rate 57600/1000000; switching includes the GD32, restored to 1M on exit |
| `SERVO_CENTER` / `SERVO_AMP` | 2048 / 284 | Center position and sweep amplitude (284 = ±25°) |
| `SERVO_VEL` / `SERVO_PERIOD` | 10 / 3 | Sweep speed and half-period seconds |
| `MIC_REC` / `MIC_PAUSE` | 5 / 5 | Record / pause seconds loop |
| `FAN_PWM` | 255 | Fan duty 0-255 |
| `DXL` / `TOF` / `NFC` | /opt/... | Test tool paths |

Examples:

```sh
BT_MAC=AA:BB:CC:DD:EE:FF sudo -E /opt/microduck-test/re-test/re-test.sh   # connect a gamepad (pairs automatically if unpaired)
DURATION=28800 sudo /opt/microduck-test/re-test/re-test.sh                  # run 8 hours then stop
SKIP="bt nfc" sudo /opt/microduck-test/re-test/re-test.sh                   # disable some modules
SERVO_BAUD=57600 sudo -E /opt/microduck-test/re-test/re-test.sh             # test with the bus at 57600, restored to 1M on exit
```

- Put the gamepad into pairing mode and the `bt` module scans→pairs→trusts→connects automatically; `bluetoothctl devices` lists paired MACs
- ToFd being stopped during the ToF test is expected; the script restarts it on exit
- For a servo that cannot hold torque due to the overvoltage alarm (err=128), first follow `dxl fixvolt` in section 24.2 of `test/hardware-test.md` (at the /opt/microduck-test/ path once this repo is preinstalled into the image)
- `SERVO_BAUD` supports 57600 / 1000000: when it differs from the target, all servos (XL330 address 8, EEPROM, torque must be off) and the GD32 (address 8, stored to flash) are switched over before the test starts;
  on exit it is always restored to 1000000 (the robot software bus default is 1M)

## Logs

`/tmp/re-test/<timestamp>/`:

- `<module>.log`: raw output of each module (servo logs include the baud-rate switch result, homing verification and periodic status snapshots)
- `state/<module>`: latest status line of each module (data source for the screen summary)
- `summary.csv`: time, SoC temperature, CPU frequency, live process count (one row per 10s)
- `main.log`: module start/stop and abnormal events

How to read the result: pass = no module repeatedly restarting, no
overheat protection below 85°C, servo positions following targets with
no non-zero `hwerr`, WiFi scans continuously seeing BSS, and Bluetooth
not dropping.
