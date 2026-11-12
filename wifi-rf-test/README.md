# WQ9201 WiFi / BT Fixed-Frequency RF Test (RSE)

The MicroDuck (RK3566) carries a Wuqi WQ9201 WiFi/BT combo chip
(Wi-Fi 6 + BT, SDIO + UART interface). This tool makes the chip transmit
continuously on a given band / channel / modulation, for RSE (radiated
spurious emissions) and other certification tests, measured with the
lab's receiver.

Verified on microduck: kernel 6.1.172-vendor-rk35xx, driver 2.2.3.94.

Command parameter definitions follow
`testScripts/rockchip/RK3576-IO-Board/wifi-module`
(github.com/Lesords/testScripts).

## Preinstalled Contents

During FULL image build this directory is copied to
`/opt/microduck-test/wifi-rf-test/` and installed system-wide by
`install.sh`, ready to use out of the box:

- MP (factory-test) firmware → `/lib/firmware/` (file names differ from
  the normal firmware; both coexist)
  - `wq9201_fw_wifi_1_1_dtest.bin`: WiFi fixed-frequency firmware
  - `wq9201_fw_dtop_1_1_mp_sdio.bin`: dtop MP firmware
  - `wq9201_fw_bt_1_1_mp.bin`: Bluetooth MP firmware
- `ioctl_app` → `/usr/bin/ioctl_app` (aarch64 static build)
- Scripts: `/opt/microduck-test/wifi-rf-test/scripts/`
  - `load_mp_ko.sh`: unload the normal driver, load the MP-firmware driver
  - `enter_mp_mode.sh`: enter factory-test verify mode (incl. RF self-calibration)
  - `tx.sh`: fixed-frequency continuous TX (parameterized)
  - `rx.sh`: fixed-frequency RX configuration and counter reset
- `reference/wq-commands.txt`: ready-made raw command blocks per
  band / rate (vendor-provided)

> While fixed-frequency mode is active, wlan0 / hci0 are completely
> unusable. RSE runs in an anechoic chamber — make sure the control
> channel is wired (Ethernet / USB gadget / serial console) and do not
> rely on WiFi for remote access.

## WiFi RSE Test Procedure

### Overall Flow

```
wire up the control link → load_mp_ko.sh → into the chamber → enter_mp_mode.sh (self-cal) →
tx.sh continuous TX → lab swept measurement → tx.sh cancel → next configuration → … →
all configurations measured → reboot to restore
```

All commands below run on the board; the script path is abbreviated as
`SCRIPTS=/opt/microduck-test/wifi-rf-test/scripts`.

### 1. Load the MP driver

```bash
sudo $SCRIPTS/load_mp_ko.sh
```

The script stops btattach, unloads `wq_wlan`, then reloads it with the
MP firmware parameters (`loadfw_only=1 fw_sys=5 force_reset=1`).

### 2. Enter factory-test mode

**Connect the antenna port first (for RSE the unit's own antennas are
fine; for conducted tests connect the RF cable to the instrument)**,
then run:

```bash
sudo $SCRIPTS/enter_mp_mode.sh
```

`dstart 15 1 0` inside is the RF self-calibration — **run it only once
per power cycle**. This step is needed only once during the whole test;
switching channel / modulation afterwards does not need re-calibration.

### 3. Continuous TX (RSE measurement state)

```bash
./tx.sh <channel> <mode> <rate> <bw> <power_dbm> [ant]
```

- channel: actual center-frequency channel number (the script converts
  per bandwidth automatically: −2 for 40M, −6 for 80M)
- mode: 0=b/g/a, 2=n, 4=ac, 5=ax
- rate: for b/g/a write the rate directly (1 2 5.5 11 6 9 12 18 24 36 48 54);
  n=0-7; ac=0-9; ax=0-11
- bw: 20/40/80 (MHz)
- ant: 1 or 2, default 1

Once started the chip transmits continuously — **keep it running** while
the lab completes the radiated swept measurement for that configuration.

### 4. Switching Test Configuration

**Always stop TX before changing channel / modulation / power**:

```bash
sudo $SCRIPTS/tx.sh cancel          # stop TX
sudo $SCRIPTS/tx.sh 6 0 11 20 26    # next configuration
```

### 5. Recommended RSE Test Configurations (WiFi)

Pick low / mid / high channels per band, each modulation at its rated
max power. Power values below are the reference values from the vendor's
command samples; the product spec / lab requirements prevail:

| Band | Mode | Channels (low/mid/high) | tx.sh example (low ch) |
|---|---|---|---|
| 2.4G | 11b 11M | 1 / 6 / 11 | `tx.sh 1 0 11 20 26` |
| 2.4G | 11g 54M | 1 / 6 / 11 | `tx.sh 1 0 54 20 26` |
| 2.4G | 11n HT20 MCS7 | 1 / 6 / 11 | `tx.sh 1 2 7 20 20` |
| 2.4G | 11ax HE20 MCS11 | 1 / 6 / 11 | `tx.sh 1 5 11 20 17` |
| 5G | 11a 54M | 36 / 64 / 149 | `tx.sh 36 0 54 20 25` |
| 5G | 11n HT20 MCS7 | 36 / 64 / 149 | `tx.sh 36 2 7 20 19` |
| 5G | 11ax HE20 MCS11 | 36 / 64 / 149 | `tx.sh 36 5 11 20 16` |

Notes:

- Choose channels per the actual certification region (e.g. CE 2.4G uses
  CH13 at the top; 5G per the certified band plan)
- Dual-antenna products must test both antennas: `tx.sh 36 5 11 20 16 2`
  (last argument is the antenna)
- If the lab requires a higher duty cycle or a single carrier (CW):
  `tx_mpdu` in tx.sh is already a near-saturated continuous stream; for a
  CW single-tone command confirm with Wuqi
- Raw commands for more combinations are in `reference/wq-commands.txt`

### 6. RX Sensitivity (if the lab needs it)

```bash
sudo $SCRIPTS/rx.sh 7 20 1           # configure channel/bandwidth/antenna and reset counters
# instrument transmits a fixed number of packets at the matching rate
sudo ioctl_app wifi "rxratecnt 0 5 11"   # query RX packet counters (args: mode rate)
```

## Bluetooth RSE Test Procedure

BT fixed-frequency mode cannot be driven with Linux commands; it needs
the Wuqi factory-test Windows host tool, connected to the board over
serial:

The host tool's command channel is the board's Linux serial console
(the default COM port, 1500000 8N1). During init it runs `su` and
similar commands on the console, so **the console must already be a root
shell**. The board has two consoles that enumerate as COM ports on
Windows — tell them apart:

- **UART2 debug port** (TP83 TX / TP84 RX / TP85 GND, 3.3V, 1500000):
  root autologin; plug in a USB-TTL adapter and use it directly —
  prefer this one
- **USB-C gadget ACM port** (ttyGS0): goes through `/bin/login` and
  needs username/password; the host tool's `su` gets swallowed by the
  login prompt and init fails with `Timeout(3s) for cmd: <su>`.
  How to tell: if the PC got an address in the gadget subnet (whose DHCP
  gateway is the board), you are on the gadget port. If that is your
  only option, first ssh into the board (user `seeed`, board address =
  that interface's gateway) and enable root autologin on that console:

  ```sh
  sudo tee /etc/systemd/system/serial-getty@ttyGS0.service.d/autologin.conf <<'EOF'
  [Service]
  ExecStart=
  ExecStart=-/sbin/agetty --noissue --autologin root --noreset --noclear --keep-baud 115200,57600,38400,9600 ttyGS0 linux
  EOF
  sudo systemctl daemon-reload
  sudo systemctl restart serial-getty@ttyGS0
  ```

  Open that COM port in a terminal app and press Enter; a
  `root@microduck:~#` prompt means it took effect. After testing, delete
  the drop-in and `daemon-reload` + restart to restore.

1. On the board run `sudo $SCRIPTS/load_mp_ko.sh` first to load the MP driver
2. Release the serial port, select it in the host tool, configure the
   paths below and initialize:

| Setting | Path |
|---|---|
| app | `/usr/bin/ioctl_app` |
| ko | `/lib/modules/$(uname -r)/updates/dkms/wq_wlan.ko` |
| PhyCfg | `/lib/firmware/wq9201_phy_1_1_4366_0102.bin` |
| FwDtop | `/lib/firmware/wq9201_fw_dtop_1_1_mp_sdio.bin` |
| FwWiFi | `/lib/firmware/wq9201_fw_wifi_1_1_dtest.bin` |
| FwBt | `/lib/firmware/wq9201_fw_bt_1_1_mp.bin` |

3. Pick the test item in the host tool (BR/EDR or BLE, fixed channel,
   TX power) and keep it transmitting for the lab to measure. Typical
   RSE configuration: low / mid / high channels (2402 / 2440 / 2480 MHz),
   DH5 packet type for BR/EDR, 1M/2M PHY for BLE, max power, measured
   separately

## Restoring Normal Mode

### Restore

**Just reboot the board**:

```bash
sudo reboot
```

> Do not try to `rmmod` the MP driver and reload the normal firmware —
> in practice, rmmod under MP mode triggers a kernel oops
> (`wq_sdio_adma_process`) and hangs the SDIO host; even power-cycling
> `wifi` cannot re-probe afterwards (error -110). Only a reboot
> recovers. After reboot the system loads the normal firmware
> automatically — nothing else to do (the MP firmware file names differ
> from the normal ones; nothing was overwritten).

### Post-Restore Verification Checklist

After reboot confirm in order:

```bash
# 1. driver loaded
lsmod | grep wq_wlan

# 2. wlan0 back
ip link show wlan0

# 3. WiFi scan / connect works
sudo nmcli device wifi rescan ifname wlan0
nmcli device wifi list ifname wlan0
nmcli device wifi connect "<SSID>" password "<PASSWORD>" ifname wlan0

# 4. Bluetooth back (btattach is restarted by its system service)
hciconfig hci0        # should be UP RUNNING, BD Address non-zero
bluetoothctl --timeout 10 -- scan on
```

## Troubleshooting

- `insmod: Invalid parameters`: the .ko does not match the kernel BTF;
  run `objcopy --remove-section=.BTF wq_wlan.ko` and load again
- No TX output: confirm `enter_mp_mode.sh` ran and the `dstart`
  self-calibration succeeded; self-calibration must run after the
  antenna / instrument (or a 50Ω load) is connected
- New configuration not taking effect: check that `tx.sh cancel` ran
  before the new parameters were sent
