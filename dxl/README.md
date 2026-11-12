# dxl

Dynamixel Protocol 2.0 bus tool for the MicroDuck D4 bus (`/dev/ttyS3`,
1 Mbps): GD32 power board (ID 200, model 0x4D44) and XL330 servos share
the same single-wire half-duplex line. Rust, zero dependencies, shipped
as an aarch64-musl binary built by the dist CI (see `build.sh`).

Covers the same ground as the `dxl.py` helper in
`test/hardware-test.md` §24.2, without needing python/pyserial on the
board.

Usage on the board:

```sh
sudo /opt/microduck-test/dxl/dxl scan              # ping IDs 0..252 @1M then @57600
sudo /opt/microduck-test/dxl/dxl ping 200          # GD32: model 19780 (0x4D44)
sudo /opt/microduck-test/dxl/dxl sensors           # GD32 block read: IMU/quat/accel/Vbat/battery mV/temp
sudo /opt/microduck-test/dxl/dxl read 1 144 2      # servo input voltage, 0.1 V/count
sudo /opt/microduck-test/dxl/dxl write 1 64 01     # torque on
sudo /opt/microduck-test/dxl/dxl move 1 60         # swing 1024<->3072 for 60s, torque off at end
sudo /opt/microduck-test/dxl/dxl fixvolt 1         # clear Shutdown bit0 (voltage) + bit1 (overheating), reboot, recheck
sudo /opt/microduck-test/dxl/dxl reboot 1
sudo /opt/microduck-test/dxl/dxl setup 2           # factory servo (ID 1 @57600) -> ID 2 @1M
sudo /opt/microduck-test/dxl/dxl configure         # batch-configure per motor.conf [groupN] sections
```

- Port and baud are set with `-p/--port` and `-b/--baud` before the
  subcommand (or the `DXL_PORT`/`DXL_BAUD` env vars); defaults are
  `/dev/ttyS3` @ `1000000`. `scan` takes ~1 min per baud: absent IDs cost
  one 0.1 s read tick each.
- A full scan runs on fresh firmware defaults too (`57600` is probed by
  default), so it finds unconfigured servos before their baud is raised.
- Works through a plain USB-TTL adapter as well (TX --1K-- DATA, RX -- DATA,
  common GND): the tool detects and strips its own TX echo.
- Status error byte follows the protocol: bit7 is the hardware-alert flag
  (read addr 70 for the cause), low 7 bits are the error code. `err=128`
  alone is not a failure.
- Exit codes: 0 ok, 1 no answer / errors, 2 usage, 3 `sensors` with IMU
  offline (status bit0 clear).
- `configure` reads `motor.conf` (next to the binary, or a path argument):
  `[motor]` sets `port` (CLI `-p`/`DXL_PORT` wins), each `[groupN]` (1..16,
  gaps allowed) maps `current_baud`/`current_id` to `target_baud`/`target_id`;
  a group is skipped when nothing answers at its current settings. After
  (re)configuration it clears Shutdown bits for latched hw errors and
  reboots the motor to apply.
