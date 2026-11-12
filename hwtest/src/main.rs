//! Menu-driven hardware test tool for the MicroDuck D4 board.
//! Mirrors test/hardware-test.md: module menu -> action menu -> params -> run.

use std::io::{self, IsTerminal, Write};
use std::process::Command;

enum Param {
    Text { prompt: &'static str, default: &'static str },
    TextOptional { prompt: &'static str, default: &'static str },
    Choice { prompt: &'static str, options: &'static [(&'static str, &'static str)] },
}

struct Action {
    title: &'static str,
    expect: &'static str,
    manual: &'static str,
    params: &'static [Param],
    cmds: &'static [&'static str],
}

enum Node {
    Action(&'static Action),
    Menu(&'static Menu),
}

impl Node {
    fn title(&self) -> &'static str {
        match self {
            Node::Action(a) => a.title,
            Node::Menu(m) => m.title,
        }
    }
}

struct Menu {
    title: &'static str,
    items: &'static [Node],
}

// ---------------------------------------------------------------- 1. SoC / CPU

static SOC_FREQ: Action = Action {
    title: "DVFS check",
    expect: "4 cores; scaling_available_frequencies lists 408000~1800000 across 7 steps; scaling_cur_freq reaches 1800000 under load",
    manual: "",
    params: &[],
    cmds: &[
        "nproc",
        "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_available_frequencies",
        "for i in 1 2 3 4; do (timeout 3 yes > /dev/null &); done; sleep 1; \
         cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
    ],
};

// ------------------------------------------------------------------ 2. Memory

static MEM_INFO: Action = Action {
    title: "Capacity check",
    expect: "Mem total ~963Mi (1GB die)",
    manual: "",
    params: &[],
    cmds: &["free -h"],
};

// ------------------------------------------------------------------ 3. eMMC

static EMMC_INFO: Action = Action {
    title: "Capacity / model",
    expect: "mmcblk0 capacity ~58G (64GB die)",
    manual: "",
    params: &[],
    cmds: &[
        "lsblk -d -o NAME,SIZE,MODEL",
        "cat /sys/block/mmcblk0/device/name",
    ],
};

static EMMC_READ: Action = Action {
    title: "Read speed test",
    expect: "Direct-read speed at least 80 MB/s",
    manual: "",
    params: &[Param::Text { prompt: "Read size (MB)", default: "256" }],
    cmds: &["dd if=/dev/mmcblk0 of=/dev/null bs=1M count={0} iflag=direct"],
};

static EMMC_MENU: Menu = Menu {
    title: "eMMC",
    items: &[Node::Action(&EMMC_INFO), Node::Action(&EMMC_READ)],
};

// ------------------------------------------------------------------ 4. PMIC

static PMIC: Action = Action {
    title: "Power rail check",
    expect: "0x20(RK809)/0x41(RK860) show UU on i2c-0; vdd_cpu 0.9~1.0V, vdd_logic 0.875V, \
             vdd_gpu/vdd_npu 0.9V, vcc_ddr 0.5V, vcc_1v8/vcca_1v8 1.8V, vccio_sd 3.3V, \
             cam_1v2 1.2V, cam_2v8 2.8V",
    manual: "",
    params: &[],
    cmds: &[
        "sudo i2cdetect -y -r 0",
        "for r in /sys/class/regulator/regulator.*/name; do n=$(cat $r); \
         case $n in vdd_cpu|vdd_logic|vdd_gpu|vdd_npu|vcc_ddr|vcc_1v8|vcca_1v8|vccio_sd|cam_1v2|cam_2v8) \
         echo \"$n $(cat ${r%name}microvolts)\";; esac; done",
    ],
};

// --------------------------------------------------------------- 5. Power input

static POWER_IN: Action = Action {
    title: "Input stage voltages",
    expect: "vcc5v0_sys=5000000，vcc3v3_sys=3300000",
    manual: "",
    params: &[],
    cmds: &[
        "for n in vcc5v0_sys vcc3v3_sys; do \
         for r in /sys/class/regulator/regulator.*/name; do \
         [ \"$(cat $r)\" = \"$n\" ] && echo \"$n $(cat ${r%name}microvolts)\"; \
         done; done",
    ],
};

// --------------------------------------------------------------------- RTC

static RTC: Action = Action {
    title: "RTC time consistency",
    expect: "hwclock and date agree (within < 1s)",
    manual: "",
    params: &[],
    cmds: &["sudo hwclock --systohc && sudo hwclock --show && date"],
};

// ------------------------------------------------------------ 7. Temperature / stress

static TEMP_NOW: Action = Action {
    title: "Current temperature",
    expect: "Output is millidegrees C (e.g. 45000 = 45C)",
    manual: "",
    params: &[],
    cmds: &["cat /sys/class/thermal/thermal_zone0/temp /sys/class/thermal/thermal_zone1/temp"],
};

static CPU_STRESS: Action = Action {
    title: "CPU full-load stress",
    expect: "stress-ng reports successful run completed; under load SoC temp <=80C, freq >=1.4GHz",
    manual: "",
    params: &[Param::Text { prompt: "Stress duration (s)", default: "30" }],
    cmds: &[
        "cat /sys/class/thermal/thermal_zone0/temp",
        "sudo stress-ng --cpu 4 --timeout {0}",
        "cat /sys/class/thermal/thermal_zone0/temp /sys/class/thermal/thermal_zone1/temp; \
         cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
    ],
};

static THERMAL_MENU: Menu = Menu {
    title: "Temperature & stress",
    items: &[Node::Action(&TEMP_NOW), Node::Action(&CPU_STRESS)],
};

// -------------------------------------------------------------------- WiFi

static WIFI_STATUS: Action = Action {
    title: "Connection status",
    expect: "When connected shows SSID, signal better than -70dBm, tx/rx bitrate in the hundreds of Mbps (HE)",
    manual: "",
    params: &[],
    cmds: &["sudo iw dev wlan0 link"],
};

static WIFI_SCAN: Action = Action {
    title: "Scan for APs",
    expect: "Lists surrounding APs with signal strength",
    manual: "",
    params: &[],
    // "| cat": nmcli pages through less when stdout is a tty
    cmds: &["sudo nmcli -f SSID,CHAN,SIGNAL,SECURITY dev wifi list | cat"],
};

static WIFI_CONNECT: Action = Action {
    title: "Connect to an AP",
    expect: "Reports successfully activated; verify afterwards with the ping test",
    manual: "",
    params: &[
        Param::Text { prompt: "SSID", default: "" },
        Param::Text { prompt: "Password", default: "" },
    ],
    cmds: &[r#"sudo nmcli dev wifi connect "{0}" password "{1}""#],
};

static WIFI_DISCONNECT: Action = Action {
    title: "Disconnect",
    // device disconnect (rather than con down) stops NetworkManager from auto-reconnecting, handy for repeating the connect flow
    expect: "Reports successfully disconnected; connection status becomes disconnected",
    manual: "",
    params: &[],
    cmds: &["sudo nmcli dev disconnect wlan0"],
};

static WIFI_PING: Action = Action {
    title: "Ping test",
    expect: "0% packet loss",
    manual: "",
    params: &[Param::Text { prompt: "Target address", default: "one.one.one.one" }],
    cmds: &["ping -c4 {0}"],
};

static WIFI_MENU: Menu = Menu {
    title: "WiFi",
    items: &[
        Node::Action(&WIFI_STATUS),
        Node::Action(&WIFI_SCAN),
        Node::Action(&WIFI_CONNECT),
        Node::Action(&WIFI_DISCONNECT),
        Node::Action(&WIFI_PING),
    ],
};

// -------------------------------------------------------------------- Bluetooth

static BT_STATUS: Action = Action {
    title: "Controller status",
    expect: "hci0 UP RUNNING，Bus: UART",
    manual: "",
    params: &[],
    cmds: &["sudo hciconfig -a"],
};

static BT_SCAN: Action = Action {
    title: "Scan for devices",
    expect: "Discovers nearby BLE/BT devices",
    manual: "",
    params: &[Param::Text { prompt: "Scan duration (s)", default: "6" }],
    cmds: &["sudo bluetoothctl --timeout {0} scan on"],
};

static BT_PAD: Action = Action {
    title: "Gamepad connection and buttons",
    expect: "After selecting a gamepad it pairs and connects and an input device node appears; button/stick events print continuously",
    manual: "Put the gamepad into pairing mode first; after a device is selected it auto-pairs/connects and prints button events, \
             ends when the pad disconnects or on Ctrl-C",
    params: &[Param::Text { prompt: "Scan duration (s)", default: "8" }],
    cmds: &[BT_PAD_CMD],
};

const BT_PAD_CMD: &str = r#"sudo bash <<'EOS'
# Switch Pro clones pair without bonding; allow non-bonded HID via uhid
conf=/etc/bluetooth/input.conf
if ! grep -q "^ClassicBondedOnly=false" "$conf"; then
    sed -i -e 's/^#\?UserspaceHID=.*/UserspaceHID=true/' \
           -e 's/^#\?ClassicBondedOnly=.*/ClassicBondedOnly=false/' "$conf"
    systemctl restart bluetooth
    sleep 2
fi
rfkill unblock bluetooth 2>/dev/null || true
for i in 1 2 3 4 5; do
    bluetoothctl power on > /dev/null 2>&1 && break
    sleep 1
done
echo "Scanning ({0}s)..."
bluetoothctl --timeout {0} scan on > /dev/null 2>&1
mapfile -t devs < <(bluetoothctl devices | grep -iE 'controller|gamepad|xbox|8bitdo|joy-con|dualshock|dualsense')
if [ ${#devs[@]} -eq 0 ]; then
    echo "No gamepad-like devices found; listing all named devices:"
    mapfile -t devs < <(bluetoothctl devices | awk '{print $2, $3}' | grep -vE '^([0-9A-Fa-f:]{17}) ([0-9A-Fa-f]{2}-){5}')
fi
if [ ${#devs[@]} -eq 0 ]; then
    echo "No devices found; confirm the gamepad is in pairing mode and retry"
    exit 1
fi
i=1
for d in "${devs[@]}"; do echo "  $i) $d"; i=$((i+1)); done
read -rp "Select gamepad number: " n < /dev/tty
sel=${devs[$((n-1))]}
[ -z "$sel" ] && { echo "Invalid number"; exit 1; }
MAC=$(echo "$sel" | awk '{print $2}' | grep -oE '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}')
echo "Selected: $MAC"
bluetoothctl pair "$MAC" || true
bluetoothctl trust "$MAC" > /dev/null
if ! bluetoothctl connect "$MAC"; then
    echo "Connect failed; clearing the old pairing and retrying (keep the gamepad in pairing mode)"
    bluetoothctl remove "$MAC" > /dev/null
    bluetoothctl --timeout {0} scan on > /dev/null 2>&1
    bluetoothctl pair "$MAC" || { echo "Re-pairing failed"; exit 1; }
    bluetoothctl trust "$MAC" > /dev/null
    bluetoothctl connect "$MAC" || { echo "Connect failed"; exit 1; }
fi
ev=""
for i in $(seq 1 10); do
    for e in /sys/class/input/event*/device; do
        u=$(cat "$e/uniq" 2>/dev/null)
        n_=$(cat "$e/name" 2>/dev/null)
        case "$n_" in *IMU*|*Motion*) continue;; esac
        if [ "${u^^}" = "${MAC^^}" ]; then ev=/dev/input/$(basename "$(dirname "$e")"); break 2; fi
    done
    sleep 1
done
[ -z "$ev" ] && { echo "Gamepad input device not found (HID connection failed)"; exit 1; }
echo "Input device: $ev ($(cat /sys/class/input/"$(basename "$ev")"/device/name))"
echo "Press gamepad buttons to see events; ends when the pad disconnects or on Ctrl-C"
evtest "$ev"
EOS"#;

static BT_MENU: Menu = Menu {
    title: "Bluetooth",
    items: &[Node::Action(&BT_STATUS), Node::Action(&BT_SCAN), Node::Action(&BT_PAD)],
};

// --------------------------------------------------------------- USB-C OTG

static OTG: Action = Action {
    title: "OTG gadget status",
    expect: "state=configured (host attached); the gadget dir contains rockchip",
    manual: "",
    params: &[],
    cmds: &[
        "cat /sys/class/udc/fcc00000.usb/state",
        "ls /sys/kernel/config/usb_gadget/",
    ],
};

// -------------------------------------------------------------------- Audio

static SPK_PLAY: Action = Action {
    title: "Speaker playback",
    expect: "aplay -l lists card 0: rockchip,rk809-codec; playback without errors, audible tone from the speaker",
    manual: "",
    params: &[
        Param::Text { prompt: "Frequency (Hz)", default: "880" },
        Param::Text { prompt: "Duration (s)", default: "2" },
    ],
    cmds: &[
        "aplay -l",
        "amixer -c 0 sset 'Playback Path' SPK_HP",
        "amixer -c 0 sset 'DAC' 200",
        r#"ffmpeg -y -f lavfi -i "sine=frequency={0}:duration={1}" -ar 48000 -ac 2 /tmp/t.wav"#,
        "aplay -D plughw:0,0 /tmp/t.wav",
    ],
};

static MIC_REC: Action = Action {
    title: "PDM microphone recording",
    expect: "Records fully without errors; non-zero sample peak (~1e8-1e9 after softvol +20dB)",
    manual: "-D mic goes through the fixed softvol gain; deploy alsa-config first (already in the full image); \
             the PDM controller needs at least 2 channels, so -c 2 is mandatory; without facemic check lsmod | grep dmic, \
             and sudo modprobe snd-soc-dmic if needed",
    params: &[Param::Text { prompt: "Recording duration (s)", default: "3" }],
    cmds: &[
        "arecord -l",
        "sudo arecord -D mic -f S32_LE -r 16000 -c 2 -d {0} /tmp/mic-test.wav",
        r#"command -v python3 >/dev/null && python3 -c "import struct; d=open('/tmp/mic-test.wav','rb').read()[44:]; s=struct.unpack('<%di'%(len(d)//4), d); print('peak =', max(map(abs, s)))" || true"#,
    ],
};

static AUDIO_MENU: Menu = Menu {
    title: "Audio",
    items: &[Node::Action(&SPK_PLAY), Node::Action(&MIC_REC), Node::Action(&AUDIO_LOOPBACK)],
};

// Runs via dispatch (custom flow + Rust-side comparison), not the generic runner.
static AUDIO_LOOPBACK: Action = Action {
    title: "Audio loopback (play->record->compare->playback)",
    expect: "",
    manual: "Records while playing the selected audio, compares similarity, then plays the recording back",
    params: &[
        Param::Choice {
            prompt: "Audio to play",
            options: &[("Test audio bbno.wav", ""), ("Custom path", "custom")],
        },
        Param::TextOptional { prompt: "Custom audio path (press Enter directly for the test audio)", default: "" },
        Param::Text { prompt: "Play/record duration (s)", default: "10" },
    ],
    cmds: &[],
};

// ------------------------------------------------------------------ Camera

static CAM_TOPO: Action = Action {
    title: "Topology check",
    expect: "media topology contains gc2093 and gc2613 entities (the unplugged one reporting -EBUSY is expected)",
    manual: "",
    params: &[],
    cmds: &["sudo media-ctl -d /dev/media0 -p | grep -E 'gc2093|gc2613'"],
};

static CAM_GRAB: Action = Action {
    title: "Grab raw frames",
    expect: "No dropped frames; file size = frames x 3110400 bytes (NV12 1920x1080)",
    manual: "With rkaiq_3A_server running (pgrep rkaiq_3A_server), brightness/color auto-converges",
    params: &[Param::Text { prompt: "Frames", default: "30" }],
    cmds: &[
        "sudo v4l2-ctl -d /dev/video0 --stream-mmap=3 --stream-count={0} --stream-to=/tmp/f.raw",
        "ls -l /tmp/f.raw",
    ],
};

static CAM_60FPS: Action = Action {
    title: "60fps switch verification",
    expect: "120 frames complete (373248000 bytes), measured rate >=50fps; restored to 30fps afterwards",
    manual: "Must be set with no active stream",
    params: &[],
    cmds: &[
        "SUBDEV=$(grep -l -E 'gc2093|gc2613' /sys/class/video4linux/v4l-subdev*/name | head -1 | xargs dirname | xargs basename); \
         sudo v4l2-ctl -d /dev/$SUBDEV --set-subdev-fps pad=0,fps=60 && \
         sudo v4l2-ctl -d /dev/video0 --stream-mmap=3 --stream-count=120 --stream-to=/tmp/f60.raw; \
         sudo v4l2-ctl -d /dev/$SUBDEV --set-subdev-fps pad=0,fps=30",
        "ls -l /tmp/f60.raw",
    ],
};

static CAMERA_MENU: Menu = Menu {
    title: "CSI camera",
    items: &[Node::Action(&CAM_TOPO), Node::Action(&CAM_GRAB), Node::Action(&CAM_60FPS)],
};

// -------------------------------------------------------------- H.264 encoding

static H264_ENC: Action = Action {
    title: "H.264 hardware encoding",
    expect: "Encoding succeeds, prints the average frame rate (~30fps), non-empty /tmp/test.h264 produced",
    manual: "Depends on /tmp/f.raw from the camera grab (NV12 1920x1080)",
    params: &[Param::Text { prompt: "Frames to encode", default: "30" }],
    cmds: &[
        "sudo mpi_enc_test -i /tmp/f.raw -w 1920 -h 1080 -f 0 -t 7 -o /tmp/test.h264 -n {0}",
    ],
};

// ------------------------------------------------------- GPU/VPU/RGA2/NPU

static GPU_VPU: Action = Action {
    title: "Device node check",
    expect: "dri has card0/card1/renderD128/renderD129; /dev/mpp_service exists; \
             devfreq lists dmc/npu/gpu/rkvenc/rkvdec; dmesg shows rga2 probe successfully",
    manual: "",
    params: &[],
    cmds: &[
        "ls /dev/dri/ /dev/mpp_service",
        "ls /sys/class/devfreq/",
        "dmesg | grep rga2",
    ],
};

// ----------------------------------------------------------- HWRNG/watchdog

static HWRNG: Action = Action {
    title: "HWRNG / watchdog",
    expect: "Reads 16 random bytes; /dev/watchdog0 exists",
    manual: "",
    params: &[],
    cmds: &[
        "sudo dd if=/dev/hwrng bs=16 count=1 | od -t x1",
        "ls /dev/watchdog0",
    ],
};

// ------------------------------------------------------------------ EEPROM

static EEPROM: Action = Action {
    title: "Board ID EEPROM (24C32, I2C1-0x50)",
    expect: "0x50 shows UU; sysfs exposes 4096 readable bytes (all 0xFF when left blank at the factory)",
    manual: "",
    params: &[],
    cmds: &[
        "sudo i2cdetect -y -r 1",
        "sudo od -A x -t x1z /sys/bus/i2c/devices/1-0050/eeprom | head",
    ],
};

// ------------------------------------------------------------------- Qwiic

static QWIIC: Action = Action {
    title: "Qwiic bus scan (I2C3)",
    expect: "With the face board V1.0 attached, 0x20(AW21009)/0x45(AW2023) show UU and 0x6a(IMU) is visible",
    manual: "",
    params: &[],
    cmds: &["sudo i2cdetect -y 3"],
};

// ---------------------------------------------------------------- Face LED

static LED_LIST: Action = Action {
    title: "LED list",
    expect: "All 12 nodes present: face:rgb:blue/green/red (AW2023) and face:gr1/gr2/gb/green1/green2/cam (AW21009)",
    manual: "",
    params: &[],
    cmds: &["ls /sys/class/leds/ | grep -E 'face'"],
};

static LED_ON: Action = Action {
    title: "Turn LED on / off",
    expect: "The LED lights up; goes dark after Enter",
    manual: "",
    params: &[Param::Text { prompt: "LED name (see list)", default: "face:rgb:red" }],
    cmds: &[
        "echo 255 | sudo tee /sys/class/leds/{0}/brightness > /dev/null; \
         read -p '{0} should now be lit; press Enter to turn it off: ' -r; \
         echo 0 | sudo tee /sys/class/leds/{0}/brightness > /dev/null",
    ],
};

static LED_AW21009: Action = Action {
    title: "Turn AW21009 LED on / off",
    expect: "The LED lights up; goes dark after Enter",
    manual: "face:gr1:red face:gr1:green face:green2:green face:green1:green face:gb:green face:gb:blue face:gr2:green face:gr2:red face:cam:red",
    params: &[Param::Text { prompt: "LED name (see list)", default: "face:cam:red" }],
    cmds: &[
        "echo 255 | sudo tee /sys/class/leds/{0}/brightness > /dev/null; \
         read -p '{0} should now be lit; press Enter to turn it off: ' -r; \
         echo 0 | sudo tee /sys/class/leds/{0}/brightness > /dev/null",
    ],
};

static LED_MENU: Menu = Menu {
    title: "Face board LED",
    items: &[
        Node::Action(&LED_LIST),
        Node::Action(&LED_ON),
        Node::Action(&LED_AW21009),
    ],
};

// --------------------------------------------------------------------- IMU

static IMU_ID: Action = Action {
    title: "WHO_AM_I",
    expect: "WHO_AM_I = 0x70",
    manual: "",
    params: &[],
    cmds: &["sudo i2cget -y 3 0x6a 0x0f"],
};

static IMU_READ: Action = Action {
    title: "Read gyroscope + accelerometer",
    expect: "At rest the acceleration magnitude is ~1g (0.061 mg/LSB); gyro bias near 0 (4.375 mdps/LSB)",
    manual: "The last step powers it down after reading; run it even if the readings look wrong",
    params: &[],
    cmds: &[
        "sudo i2cset -y 3 0x6a 0x12 0x01; sleep 0.1; \
         sudo i2cset -y 3 0x6a 0x12 0x44; \
         sudo i2cset -y 3 0x6a 0x10 0x06; \
         sudo i2cset -y 3 0x6a 0x11 0x06",
        "sudo i2cget -y 3 0x6a 0x1e; sudo i2cget -y 3 0x6a 0x22 i 12",
        "sudo i2cset -y 3 0x6a 0x10 0x00; sudo i2cset -y 3 0x6a 0x11 0x00",
    ],
};

static IMU_MENU: Menu = Menu {
    title: "Face board IMU",
    items: &[Node::Action(&IMU_ID), Node::Action(&IMU_READ)],
};

// --------------------------------------------------------------------- ToF

static TOF: Action = Action {
    title: "Ranging",
    expect: "is_alive passes (device_id=0xF0); init succeeds, the ranging matrix streams continuously, \
             near occlusion vs background contrast is clear; at the end the data check shows nearly N×N valid zones, \
             no mostly invalid frames",
    manual: "Stops/starts tofd automatically before/after, to avoid concurrent access wedging the sensor",
    params: &[
        Param::Text { prompt: "Frames", default: "20" },
        Param::Choice {
            prompt: "Resolution",
            options: &[("4x4", "4"), ("8x8", "8")],
        },
    ],
    cmds: &["sudo systemctl stop tofd; \
             sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 {0} {1}; \
             sudo systemctl start tofd"],
};

static TOF_FPS: Action = Action {
    title: "Frame rate verification (4x4 @ 60Hz)",
    expect: "frequency readback is 60 Hz, measured ~58-60 fps, \
             data check 16/16 valid zones, no mostly invalid frames",
    manual: "60Hz is only available at 4x4 (8x8 caps at 15Hz); stops/starts tofd automatically before/after",
    params: &[Param::Text { prompt: "Frames", default: "600" }],
    cmds: &["sudo systemctl stop tofd; \
             sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 {0} 4 60; \
             sudo systemctl start tofd"],
};

// --------------------------------------------------------------------- NFC

static ANT_OPTS: &[(&str, &str)] = &[
    ("Differential (both antennas, default)", ""),
    ("NFC1 single-ended (RFO1 = J30)", "NFC_ANT=1"),
    ("NFC2 single-ended (RFO2 = J31)", "NFC_ANT=2"),
];

static NFC_ID: Action = Action {
    title: "Smoke test (IC_ID)",
    expect: "IC_ID = 0xA9, exit code 0",
    manual: "",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-id-test"],
};

static NFC_REGS: Action = Action {
    title: "Register check",
    expect: "GENERAL[0x01]=0x0C (differential, both RFOs), REGULATOR[0x02]=0x5F, TX_DRIVER[0x03]=0x00",
    manual: "",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-test regs"],
};

static NFC_FIELD: Action = Action {
    title: "Field strength readout",
    expect: "Unloaded I steady at 75-80; with a card near the antenna I/Q shifts clearly and recovers when moved away",
    manual: "Prints once per second; Ctrl-C to stop",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-test field"],
};

static NFC_POLL: Action = Action {
    title: "Full tag polling",
    expect: "A tag on either antenna is found by that side's single-ended command, which reports the card UID (starts with E00223); \
             with the tag on J30, NFC_ANT=2 should report no tag found, and vice versa",
    manual: "Switching single-ended via NFC_ANT verifies each antenna path and rules out crosstalk/miswiring",
    params: &[
        Param::Choice { prompt: "Antenna", options: ANT_OPTS },
        Param::Text { prompt: "Poll duration (ms)", default: "8000" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test poll {1}"],
};

static NFC_READ: Action = Action {
    title: "Read block (ISO15693)",
    expect: "Prints the card UID first (starts with E00223), then the 4-byte block data",
    manual: "",
    params: &[
        Param::Choice { prompt: "Antenna", options: ANT_OPTS },
        Param::Text { prompt: "Block number", default: "0" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv read {1}"],
};

static NFC_WRITE_VERIFY: Action = Action {
    title: "Write block (write-back verify, non-destructive)",
    expect: "Prints WRITE PASS (readback matches after write), exit code 0",
    manual: "Reads the original data, writes it back and verifies; a protocol error with working reads usually means the block is locked on the card",
    params: &[
        Param::Choice { prompt: "Antenna", options: ANT_OPTS },
        Param::Text { prompt: "Block number", default: "1" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv write {1}"],
};

static NFC_WRITE_DATA: Action = Action {
    title: "Write block (specific data)",
    expect: "Prints WRITE PASS (readback matches after write), exit code 0",
    manual: "Destructive write; confirm what the block is used for before running",
    params: &[
        Param::Choice { prompt: "Antenna", options: ANT_OPTS },
        Param::Text { prompt: "Block number", default: "0" },
        Param::Text { prompt: "Data (8 hex digits, e.g. aabbccdd)", default: "" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv write {1} {2}"],
};

static NFC_WRITE_ESL: Action = Action {
    title: "Write e-ink tag (ESL)",
    expect: "Prints refresh done after uploading the 4 bitplanes; the e-ink displays the image",
    manual: "The FMSC ESL tag (1.54-inch six-color 240x240) must stay on the antenna until refresh done; \
             a custom image must be a 240x240 PNG/JPG/BMP/PPM, preferably pre-quantized to the six panel colors",
    params: &[
        Param::Choice {
            prompt: "Image",
            options: &[("Built-in test pattern (six color bars)", ""), ("Custom image path", "custom")],
        },
        Param::TextOptional { prompt: "Custom image path (press Enter directly for the built-in pattern)", default: "" },
        Param::Choice { prompt: "Antenna", options: ANT_OPTS },
    ],
    cmds: &["sudo {2} /opt/microduck-test/nfc/nfc-test write {1}"],
};

static NFC_MENU: Menu = Menu {
    title: "NFC (ST25R100)",
    items: &[
        Node::Action(&NFC_ID),
        Node::Action(&NFC_REGS),
        Node::Action(&NFC_FIELD),
        Node::Action(&NFC_POLL),
        Node::Action(&NFC_READ),
        Node::Action(&NFC_WRITE_VERIFY),
        Node::Action(&NFC_WRITE_DATA),
        Node::Action(&NFC_WRITE_ESL),
    ],
};

// --------------------------------------------------------------------- Fan

static FAN_AUTO: Action = Action {
    title: "PWM / TACH automatic test",
    expect: "Full-speed TACH ~600Hz, half ~315Hz, 0 pulses when stopped; speed control roughly linear",
    manual: "Requires the physical fan",
    params: &[],
    cmds: &[concat!(
        "pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); ",
        "[ -n \"$pwm\" ] || { echo 'no pwmfan hwmon found'; exit 1; }; ",
        "echo 1 | sudo tee ${pwm}_enable > /dev/null; ",
        "for duty in 255 128 0; do ",
        "echo $duty | sudo tee $pwm > /dev/null; sleep 1; ",
        "[ \"$duty\" = 0 ] && break; ",
        "n=$(timeout 3 sudo gpiomon -r gpiochip3 8 | wc -l); ",
        "echo \"duty=$duty tach~$((n/3))Hz\"; done"
    )],
};

static FAN_SET: Action = Action {
    title: "Set speed manually",
    expect: "pwm reads back what was written; fan speed follows",
    manual: "",
    params: &[Param::Choice {
        prompt: "Duty cycle",
        options: &[("Full (255)", "255"), ("Half (128)", "128"), ("Stop (0)", "0")],
    }],
    cmds: &[
        "pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); \
         [ -n \"$pwm\" ] || { echo 'no pwmfan hwmon found'; exit 1; }; \
         echo 1 | sudo tee ${pwm}_enable > /dev/null; \
         echo {0} | sudo tee $pwm > /dev/null; cat $pwm",
    ],
};

static FAN_MENU: Menu = Menu {
    title: "Fan",
    items: &[Node::Action(&FAN_AUTO), Node::Action(&FAN_SET)],
};

// ------------------------------------------------------------------ UART3

static UART3: Action = Action {
    title: "Port check",
    expect: "/dev/ttyS3 exists and configures cleanly",
    manual: "",
    params: &[],
    cmds: &["ls /dev/ttyS3 && stty -F /dev/ttyS3 115200"],
};

static DXL_SCAN: Action = Action {
    title: "Bus scan",
    expect: "Lists responding devices as (ID, model): GD32 power board (200, 1200), servo (1, 1200), possibly a second servo (2, 1200); the power board masquerades as an XL330 — tell it apart by ID 200",
    manual: "Scans at 1 Mbps, takes about a minute",
    params: &[],
    cmds: &["sudo /opt/microduck-test/dxl/dxl scan 1000000"],
};

static DXL_GD32: Action = Action {
    title: "GD32 power board",
    expect: "ping 10/10, model 1200 (XL330 disguise), fw 7, err=0; imu_online=true, voltage 5-20 V",
    manual: "ID 200; err=128 is just the hardware alarm flag, err=0 means no alarms",
    params: &[],
    cmds: &[
        "sudo /opt/microduck-test/dxl/dxl ping 200",
        "sudo /opt/microduck-test/dxl/dxl sensors",
    ],
};

static DXL_SERVO_PING: Action = Action {
    title: "Servo ping / status",
    expect: "ping 10/10, model 1200 (XL330); voltage reads 0.1 V per count",
    manual: "err=128 means a hardware alarm; see hw_err for the cause (bit0=voltage alarm, expected on the 8.3 V bus, does not affect operation)",
    params: &[Param::Text { prompt: "Servo ID", default: "1" }],
    cmds: &[
        "sudo /opt/microduck-test/dxl/dxl ping {0}",
        "sudo /opt/microduck-test/dxl/dxl read {0} 70 1",
        "sudo /opt/microduck-test/dxl/dxl read {0} 144 2",
    ],
};

static DXL_SERVO_MOVE: Action = Action {
    title: "Servo motion test",
    expect: "The servo sweeps the set amplitude around center, homes and unloads at the end, 0 errors",
    manual: "Watch the servo actually move; no motion or jittering is a FAIL",
    params: &[
        Param::Text { prompt: "Servo ID", default: "1" },
        Param::Text { prompt: "Duration (s)", default: "10" },
        Param::Text { prompt: "Amplitude (±deg)", default: "15" },
    ],
    cmds: &["sudo /opt/microduck-test/dxl/dxl move {0} {1} {2}"],
};

static DXL_SERVO_FIXVOLT: Action = Action {
    title: "Servo overvoltage/overheat recovery",
    expect: "Shutdown bit0/bit1 cleared and rebooted; recheck hw_err",
    manual: "Only for servos latched by overvoltage/overheat protection (Shutdown bit0/bit1 set); for overheat let the servo cool first or it trips again; on the 8.3 V bus hw_err bit0 still sets — expected",
    params: &[Param::Text { prompt: "Servo ID", default: "1" }],
    cmds: &["sudo /opt/microduck-test/dxl/dxl fixvolt {0}"],
};

static DXL_SERVO_SETUP: Action = Action {
    title: "Set up a new servo",
    expect: "Writes the target ID and 1 Mbps baud to the factory servo (ID 1 @ 57600), then pings it under the new configuration",
    manual: "Only one new servo on the bus at a time; multiple 57600 devices abort the setup",
    params: &[Param::Text { prompt: "Target ID", default: "1" }],
    cmds: &["sudo /opt/microduck-test/dxl/dxl setup {0}"],
};

static DXL_MENU: Menu = Menu {
    title: "Dynamixel bus (UART3)",
    items: &[
        Node::Action(&UART3),
        Node::Action(&DXL_SCAN),
        Node::Action(&DXL_GD32),
        Node::Action(&DXL_SERVO_PING),
        Node::Action(&DXL_SERVO_MOVE),
        Node::Action(&DXL_SERVO_FIXVOLT),
        Node::Action(&DXL_SERVO_SETUP),
    ],
};

// ------------------------------------------------------------ Recovery key

static RECOVERY_KEY: Action = Action {
    title: "Recovery key",
    expect: "raw=1023 with SW2 released; drops significantly while held",
    manual: "Download mode needs separate verification: hold SW2 during u-boot and reboot; rkdeveloptool ld on the PC should show a Loader device",
    params: &[],
    cmds: &[
        "cat /sys/bus/iio/devices/iio:device0/in_voltage0_raw; \
         read -p 'Hold SW2 down and press Enter: ' -r; \
         cat /sys/bus/iio/devices/iio:device0/in_voltage0_raw",
    ],
};

// ----------------------------------------------------------------- Maskrom

static MASKROM: Action = Action {
    title: "Maskrom test point",
    expect: "The PC enumerates the Maskrom device and flashing works",
    manual: "Requires a power cycle: with power off, short TP64 (eMMC_D0) to TP4 (GND), then power on into maskrom; \
             rkdeveloptool ld on the PC should list a Maskrom device",
    params: &[],
    cmds: &[],
};

// ------------------------------------------------------------------- HW_ID

static HW_ID: Action = Action {
    title: "HW_ID (SARADC ch1)",
    expect: "raw=1023, meaning Version 1 (Rup 10K / Rdown DNP)",
    manual: "",
    params: &[],
    cmds: &["cat /sys/bus/iio/devices/iio:device0/in_voltage1_raw"],
};

// ----------------------------------------------------------- DDR/CPU stress

static MEMTESTER: Action = Action {
    title: "memtester (single full pass)",
    expect: "Stuck Address / Random Value / XOR / Walking Ones etc. all ok",
    manual: "",
    params: &[Param::Text { prompt: "Test size", default: "128M" }],
    cmds: &["sudo memtester {0} 1"],
};

static DDR_FREQ: Action = Action {
    title: "DDR frequency switch",
    expect: "Prints success change to ... Hz; cur_freq matches the target",
    manual: "Run 'Restore DDR DVFS' afterwards",
    params: &[Param::Choice {
        prompt: "Target frequency",
        options: &[
            ("324 MHz", "324000000"),
            ("528 MHz", "528000000"),
            ("780 MHz", "780000000"),
            ("1056 MHz", "1056000000"),
        ],
    }],
    cmds: &[
        "sudo bash /opt/microduck-test/rockchip-test/ddr/ddr_freq_scaling.sh {0}; \
         cat /sys/class/devfreq/dmc/cur_freq",
    ],
};

static DDR_FREQ_RESTORE: Action = Action {
    title: "Restore DDR DVFS (dmc_ondemand)",
    expect: "governor back to dmc_ondemand",
    manual: "",
    params: &[],
    cmds: &["echo dmc_ondemand | sudo tee /sys/class/devfreq/dmc/governor"],
};

static CPU_FREQ_STRESS: Action = Action {
    title: "CPU frequency stress (~100 seconds)",
    expect: "Each step prints successfully changed; stress-ng reports successful run completed; \
             governor restored to ondemand afterwards",
    manual: "Steps through frequencies under load",
    params: &[],
    cmds: &[
        "sudo stress-ng --cpu 4 --timeout 60 & sleep 2; \
         for f in 408000 816000 1416000 1800000 600000 1104000 1608000; do \
         sudo bash /opt/microduck-test/rockchip-test/cpu/cpu_freq_scaling.sh $f; sleep 5; done; \
         echo ondemand | sudo tee /sys/devices/system/cpu/cpufreq/policy0/scaling_governor; wait",
    ],
};

static STRESSAPP: Action = Action {
    title: "stressapptest (DDR stress)",
    expect: "Status: PASS, 0 errors",
    manual: "24h recommended for mass production",
    params: &[Param::Text { prompt: "Stress duration (s)", default: "300" }],
    cmds: &[
        "sudo mkdir -p /userdata/rockchip-test && \
         sudo stressapptest -s {0} -i 4 -C 4 -W --stop_on_errors -M 128 \
         -l /userdata/rockchip-test/stressapptest.log",
    ],
};

static STRESS_MENU: Menu = Menu {
    title: "DDR / CPU stress",
    items: &[
        Node::Action(&MEMTESTER),
        Node::Action(&DDR_FREQ),
        Node::Action(&DDR_FREQ_RESTORE),
        Node::Action(&CPU_FREQ_STRESS),
        Node::Action(&STRESSAPP),
    ],
};

// ---------------------------------------------------------------- Auto full check

// Runs via run_auto_check (quiet, auto-judged), not the generic runner.
static AUTO_CHECK: Action = Action {
    title: "Auto full check",
    expect: "",
    manual: "Fully automatic, no input needed; attach one NFC tag to each of the J30 / J31 antennas beforehand.\n\
             During the run all LEDs fade dim->bright->dim together and the fan sweeps low->high; afterwards the LEDs go dark and the fan stays at full speed.",
    params: &[],
    cmds: &[],
};

// ----------------------------------------------------------------- Main menu

static MAIN_MENU: Menu = Menu {
    title: "MicroDuck D4 hardware test",
    items: &[
        Node::Menu(&Menu { title: "SoC / CPU DVFS", items: &[Node::Action(&SOC_FREQ)] }),
        Node::Menu(&Menu { title: "Memory", items: &[Node::Action(&MEM_INFO)] }),
        Node::Menu(&EMMC_MENU),
        Node::Menu(&Menu { title: "PMIC power rails", items: &[Node::Action(&PMIC)] }),
        Node::Menu(&Menu { title: "Power input stage", items: &[Node::Action(&POWER_IN)] }),
        Node::Menu(&Menu { title: "RTC", items: &[Node::Action(&RTC)] }),
        Node::Menu(&THERMAL_MENU),
        Node::Menu(&WIFI_MENU),
        Node::Menu(&BT_MENU),
        Node::Menu(&Menu { title: "USB-C OTG", items: &[Node::Action(&OTG)] }),
        Node::Menu(&AUDIO_MENU),
        Node::Menu(&CAMERA_MENU),
        Node::Menu(&Menu { title: "H.264 hardware encoding", items: &[Node::Action(&H264_ENC)] }),
        Node::Menu(&Menu { title: "GPU / VPU / RGA2 / NPU", items: &[Node::Action(&GPU_VPU)] }),
        Node::Menu(&Menu { title: "HWRNG / watchdog", items: &[Node::Action(&HWRNG)] }),
        Node::Menu(&Menu { title: "Board ID EEPROM", items: &[Node::Action(&EEPROM)] }),
        Node::Menu(&Menu { title: "Qwiic (I2C3)", items: &[Node::Action(&QWIIC)] }),
        Node::Menu(&LED_MENU),
        Node::Menu(&IMU_MENU),
        Node::Menu(&Menu { title: "Face board ToF", items: &[Node::Action(&TOF), Node::Action(&TOF_FPS)] }),
        Node::Menu(&NFC_MENU),
        Node::Menu(&FAN_MENU),
        Node::Menu(&DXL_MENU),
        Node::Menu(&Menu { title: "Recovery key / download mode", items: &[Node::Action(&RECOVERY_KEY)] }),
        Node::Menu(&Menu { title: "Maskrom test point", items: &[Node::Action(&MASKROM)] }),
        Node::Menu(&Menu { title: "HW_ID", items: &[Node::Action(&HW_ID)] }),
        Node::Menu(&STRESS_MENU),
        Node::Menu(&Menu { title: "Auto full check", items: &[Node::Action(&AUTO_CHECK)] }),
    ],
};

// ------------------------------------------------------------------ Runtime

#[derive(Clone, Copy)]
enum Verdict {
    Pass,
    Fail,
    Skip,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Skip => "SKIP",
        }
    }

    fn colored(self) -> String {
        let code = match self {
            Verdict::Pass => "32",
            Verdict::Fail => "31;1",
            Verdict::Skip => "33",
        };
        if io::stdout().is_terminal() {
            format!("\x1b[{code}m{}\x1b[0m", self.label())
        } else {
            self.label().to_string()
        }
    }
}

enum Nav {
    Back,
    Main,
    Quit,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        None => {}
        Some("--list") | Some("-l") => {
            print_menu_tree(&MAIN_MENU, 0);
            return;
        }
        Some("--help") | Some("-h") => usage(&args[0]),
        Some(other) => {
            eprintln!("unknown argument: {other}");
            usage(&args[0]);
        }
    }

    if !is_root() {
        println!("!! Not root; some items need sudo; preferably run as sudo hwtest");
    }

    let mut results: Vec<(String, Verdict)> = Vec::new();
    loop {
        match menu_loop(&MAIN_MENU, "", &mut results, 0) {
            Nav::Quit | Nav::Back => break,
            Nav::Main => continue,
        }
    }

    if results.is_empty() {
        return;
    }
    println!("\n===== Test Summary =====");
    let mut report = String::new();
    for (label, verdict) in &results {
        println!("{:<48} {}", label, verdict.colored());
        let line = format!("{:<48} {}", label, verdict.label());
        report.push_str(&line);
        report.push('\n');
    }
    let path = "/tmp/hwtest-report.txt";
    match std::fs::write(path, &report) {
        Ok(()) => println!("\nReport written to {path}"),
        Err(e) => eprintln!("Cannot write {path}: {e}"),
    }
}

fn usage(prog: &str) -> ! {
    eprintln!("usage: {prog}         interactive menu");
    eprintln!("       {prog} --list  print the test item tree");
    std::process::exit(2);
}

fn print_menu_tree(menu: &Menu, indent: usize) {
    println!("{}{}", "  ".repeat(indent), menu.title);
    for item in menu.items {
        match item {
            Node::Action(a) => println!("{}- {}", "  ".repeat(indent + 1), a.title),
            Node::Menu(m) => print_menu_tree(m, indent + 1),
        }
    }
}

fn menu_loop(menu: &'static Menu, crumb: &str, results: &mut Vec<(String, Verdict)>, depth: usize) -> Nav {
    // Single-action module: run it directly instead of showing a submenu.
    if let [Node::Action(action)] = menu.items {
        dispatch(action, crumb, results);
        return Nav::Back;
    }
    loop {
        println!("\n== {} ==", if crumb.is_empty() { menu.title } else { crumb });
        for (i, item) in menu.items.iter().enumerate() {
            println!("{:>2}. {}", i + 1, item.title());
        }
        let hint = if depth == 0 {
            "pick a number, q to quit"
        } else {
            "pick a number, b back, m main menu, q to quit"
        };
        match prompt(&format!("{hint}: ")).as_str() {
            "q" | "Q" => return Nav::Quit,
            "b" | "B" if depth > 0 => return Nav::Back,
            "m" | "M" if depth > 0 => return Nav::Main,
            input => match input.parse::<usize>() {
                Ok(n) if n >= 1 && n <= menu.items.len() => match menu.items[n - 1] {
                    Node::Action(action) => {
                        let label = format!("{crumb} / {}", action.title);
                        dispatch(action, &label, results);
                    }
                    Node::Menu(sub) => {
                        let crumb = format!("{}{}{}", crumb, if crumb.is_empty() { "" } else { " / " }, sub.title);
                        match menu_loop(sub, &crumb, results, depth + 1) {
                            Nav::Back => {}
                            nav => return nav,
                        }
                    }
                },
                _ => println!("Invalid input, try again"),
            },
        }
    }
}

// The auto check has its own quiet, auto-judged runner; everything else
// uses the generic command-steps runner.
fn dispatch(action: &'static Action, label: &str, results: &mut Vec<(String, Verdict)>) {
    if std::ptr::eq(action, &AUTO_CHECK) {
        run_auto_check(results);
    } else if std::ptr::eq(action, &AUDIO_LOOPBACK) {
        run_audio_loopback(action, label, results);
    } else {
        run_action(action, label, results);
    }
}

// Audio menu loopback test: pick the audio, play while recording, compare
// similarity, then play the recording back.
fn run_audio_loopback(action: &Action, label: &str, results: &mut Vec<(String, Verdict)>) {
    fn run(cmd: &str) -> bool {
        println!("$ {cmd}");
        matches!(Command::new("bash").arg("-c").arg(cmd).status(), Ok(s) if s.success())
    }

    println!("\n===== {label} =====");
    println!("Note: {}", action.manual);
    let mut args: Vec<String> = Vec::new();
    for param in action.params {
        match read_param(param) {
            Some(value) => args.push(value),
            None => {
                println!("Cancelled");
                return;
            }
        }
    }
    let wav = if args[0] != "custom" {
        if std::path::Path::new("/opt/microduck-test/hwtest/bbno.wav").exists() {
            "/opt/microduck-test/hwtest/bbno.wav".to_string()
        } else {
            "./bbno.wav".to_string()
        }
    } else {
        args[1].clone()
    };
    let dur = &args[2];

    let setup = run("amixer -q -c 0 sset 'Playback Path' SPK_HP && \
         amixer -q -c 0 sset 'DAC' 200 && amixer -q -c 0 sset 'HP Output Gain' 3");
    println!("Playing and recording {dur}s...");
    let played = run(&format!(
        "rm -f /tmp/mic-rec.wav; \
         aplay -q -D plughw:0,0 -d {dur} \"{wav}\" & \
         sudo arecord -q -D mic -f S32_LE -r 16000 -c 2 -d {dur} /tmp/mic-rec.wav; wait"
    ));
    let mut ok = setup && played;
    match compare_audio(&wav, "/tmp/mic-rec.wav") {
        Some((peak, similarity)) => {
            ok = ok && peak > 1e6 / 2f32.powi(31) && similarity >= 0.4;
            println!(
                "Recording peak {:.3e}, similarity vs original {:.0}% (reference: >=40% similarity is good)",
                peak * 2f32.powi(31),
                similarity * 100.0
            );
        }
        None => {
            ok = false;
            println!("Failed to parse the audio file");
        }
    }
    println!("Playing the recording back...");
    run("aplay -q -D plughw:0,0 /tmp/mic-rec.wav");
    // The similarity auto-verdict is advisory; the final call is what the playback sounds like
    println!("Similarity auto-verdict: {}", if ok { Verdict::Pass.colored() } else { Verdict::Fail.colored() });
    let verdict = loop {
        match prompt("Verdict [P]ass / [F]ail / [S]kip (default s): ").to_ascii_lowercase().as_str() {
            "p" | "pass" => break Verdict::Pass,
            "f" | "fail" => break Verdict::Fail,
            "" | "s" | "skip" => break Verdict::Skip,
            _ => println!("Enter p / f / s"),
        }
    };
    println!("Result: {}\n", verdict.colored());
    results.push((label.to_string(), verdict));
}

// Auto full check: runs every step quietly, judges the result itself and
// prints one colored PASS/FAILED line per step. No operator input needed.
fn run_auto_check(results: &mut Vec<(String, Verdict)>) {
    fn run(cmd: &str) -> (bool, String) {
        match Command::new("bash").arg("-c").arg(cmd).output() {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                (out.status.success(), text)
            }
            Err(e) => (false, e.to_string()),
        }
    }

    println!("\n===== Auto Full Check =====");
    println!("Note: {}", AUTO_CHECK.manual);

    let start = results.len();
    let mut index = 0u32;
    let mut report = |name: &str, ok: bool, detail: &str, results: &mut Vec<(String, Verdict)>| {
        index += 1;
        let verdict = if ok { Verdict::Pass } else { Verdict::Fail };
        println!("[{index}] {name} ... {}", verdict.colored());
        let detail = detail.trim();
        if !detail.is_empty() {
            println!("    {detail}\n");
        } else {
            println!();
        }
        results.push((format!("Auto full check / {name}"), verdict));
    };

    // Background effects for the whole sequence: LED fade in/out, fan sweep.
    // Loops carry a marker string so cleanup can pkill -f them; $! would be
    // wrong when setsid forks.
    run(r#"for f in /sys/class/leds/face:*/trigger; do echo none > $f 2>/dev/null; done; \
         setsid bash -c ': hwtest-led-loop; while :; do \
         br=0; while [ $br -lt 255 ]; do br=$((br+15)); [ $br -gt 255 ] && br=255; \
         for f in /sys/class/leds/face:*/brightness; do echo $br > $f 2>/dev/null; done; sleep 0.02; done; \
         br=255; while [ $br -gt 0 ]; do br=$((br-15)); [ $br -lt 0 ] && br=0; \
         for f in /sys/class/leds/face:*/brightness; do echo $br > $f 2>/dev/null; done; sleep 0.02; done; \
         done' >/dev/null 2>&1 &"#);
    run(r#"pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); \
         [ -n "$pwm" ] && { echo 1 > ${pwm}_enable; \
         setsid bash -c ': hwtest-fan-loop; while :; do for d in 0 64 128 192 255 192 128 64; do \
         echo $d > '"$pwm"'; sleep 2; done; done' >/dev/null 2>&1 & }"#);

    // WiFi scan
    let (ok, out) = run("sudo nmcli -t -f SSID dev wifi list --rescan yes 2>/dev/null | grep -c .");
    report("WiFi scan", ok, &format!("found {} APs (PASS if: >=1)", out.trim()), results);

    // Bluetooth scan
    let (up, _) = run("sudo hciconfig hci0 2>/dev/null | grep -q 'UP RUNNING'");
    let (_, out) = run("sudo bluetoothctl --timeout 5 scan on 2>/dev/null | \
         grep -oiE 'device ([0-9a-f]{2}:){5}[0-9a-f]{2}' | sort -u | wc -l");
    report(
        "Bluetooth scan",
        up,
        &format!(
            "controller {}, found {} devices (PASS if: controller UP RUNNING)",
            if up { "UP RUNNING" } else { "DOWN" },
            out.trim()
        ),
        results,
    );

    // IMU readings
    let (_, id) = run("sudo i2cget -y 3 0x6a 0x0f 2>/dev/null");
    let id = id.trim().to_string();
    if id == "0x70" {
        run("sudo i2cset -y 3 0x6a 0x12 0x01; sleep 0.1; \
             sudo i2cset -y 3 0x6a 0x12 0x44; \
             sudo i2cset -y 3 0x6a 0x10 0x06; sudo i2cset -y 3 0x6a 0x11 0x06");
        let (_, data) = run("sudo i2cget -y 3 0x6a 0x22 i 12 2>/dev/null");
        run("sudo i2cset -y 3 0x6a 0x10 0x00; sudo i2cset -y 3 0x6a 0x11 0x00");
        report(
            "IMU readings",
            true,
            &format!("WHO_AM_I={id} (PASS if: ==0x70)\n    gyro+accel: {}", data.trim()),
            results,
        );
    } else {
        report(
            "IMU readings",
            false,
            &format!("WHO_AM_I={id} (expected 0x70; unreadable usually means the face board is absent or I2C3 is broken)"),
            results,
        );
    }

    // Camera photo: 10 frames, keep the last (3A converged), convert to jpg
    let (ok, _) = run("sudo v4l2-ctl -d /dev/video0 --stream-mmap=3 --stream-count=10 \
         --stream-to=/tmp/photo-raw.nv12 > /dev/null 2>&1 && \
         tail -c 3110400 /tmp/photo-raw.nv12 > /tmp/photo.nv12 && \
         ffmpeg -v error -y -f rawvideo -pix_fmt nv12 -s 1920x1080 -i /tmp/photo.nv12 /tmp/photo.jpg && \
         test -s /tmp/photo.jpg");
    let (_, size) = run("stat -c %s /tmp/photo.jpg 2>/dev/null");
    report(
        "Camera photo",
        ok,
        &format!("/tmp/photo.jpg {} bytes (PASS if: capture succeeded and the photo is non-empty)", size.trim()),
        results,
    );

    // ToF ranging
    let (ok, out) = run("sudo systemctl stop tofd; \
         sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 5 4 2>&1; \
         rc=$?; sudo systemctl start tofd; exit $rc");
    report("ToF ranging", ok, &format!("{}\n    (PASS if: init ok and exit code 0)", out.trim()), results);

    // GD32 power board over Dynamixel: dxl sensors exits 0 only when the
    // GD32 answers with err=0 and the IMU is online (exit 3 = IMU offline)
    let (ok, out) = run("sudo /opt/microduck-test/dxl/dxl sensors 2>&1");
    report(
        "GD32 power board (Dynamixel)",
        ok,
        &format!("{}\n    (PASS if: ID 200 answers, err=0, imu_online=true)", out.trim()),
        results,
    );

    // EEPROM (24C32): write random 16 bytes at the tail (keep clear of the
    // board-ID area), read back, then restore the original data
    let (ok, out) = run(r#"e=/sys/bus/i2c/devices/1-0050/eeprom; [ -e "$e" ] || exit 1; \
         orig=$(dd if=$e bs=1 skip=4080 count=16 2>/dev/null | od -A n -t x1 | tr -d ' \n'); \
         pat=$(head -c16 /dev/urandom | od -A n -t x1 | tr -d ' \n'); \
         printf "$(echo $pat | sed 's/../\\x&/g')" | dd of=$e bs=1 seek=4080 conv=notrunc 2>/dev/null; \
         sleep 0.1; \
         back=$(dd if=$e bs=1 skip=4080 count=16 2>/dev/null | od -A n -t x1 | tr -d ' \n'); \
         printf "$(echo $orig | sed 's/../\\x&/g')" | dd of=$e bs=1 seek=4080 conv=notrunc 2>/dev/null; \
         sleep 0.1; \
         rest=$(dd if=$e bs=1 skip=4080 count=16 2>/dev/null | od -A n -t x1 | tr -d ' \n'); \
         echo "orig=$orig"; echo "written=$pat"; echo "readback=$back"; echo "restored=$rest"; \
         [ -n "$pat" ] && [ "$back" = "$pat" ] && [ "$rest" = "$orig" ]"#);
    report(
        "EEPROM write-read-restore",
        ok,
        &format!("{}\n    (PASS if: readback matches the write and the original data is restored)", out.trim()),
        results,
    );

    // NFC poll: one tag on each antenna, verify both single-ended paths
    let mut lines = Vec::new();
    let mut nfc_ok = true;
    for (name, ant) in [("NFC1(J30)", "1"), ("NFC2(J31)", "2")] {
        let (_, out) = run(&format!(
            "sudo NFC_ANT={ant} /opt/microduck-test/nfc/nfc-test poll 5000 2>&1"
        ));
        match out.lines().find(|l| l.contains("nfcid")) {
            Some(l) => lines.push(format!("{name}: {}", l.trim())),
            None => {
                nfc_ok = false;
                lines.push(format!("{name}: no tag found"));
            }
        }
    }
    report(
        "NFC polling (both antennas)",
        nfc_ok,
        &format!("{}\n    (PASS if: both antennas read a card UID)", lines.join("\n    ")),
        results,
    );

    // Audio loopback: record 10s while playing bbno.wav 10s, check peak,
    // then play the recording back. wav falls back to ./bbno.wav so a
    // standalone copy of the tool works without /opt layout.
    let (setup, _) = run("wav=/opt/microduck-test/hwtest/bbno.wav; [ -f $wav ] || wav=./bbno.wav; \
         test -f $wav && amixer -q -c 0 sset 'Playback Path' SPK_HP && \
         amixer -q -c 0 sset 'DAC' 200 && amixer -q -c 0 sset 'HP Output Gain' 3");
    let (played, _) = run("wav=/opt/microduck-test/hwtest/bbno.wav; [ -f $wav ] || wav=./bbno.wav; \
         aplay -q -D plughw:0,0 -d 10 $wav & \
         sudo arecord -q -D mic -f S32_LE -r 16000 -c 2 -d 10 /tmp/mic-rec.wav; wait");
    run("aplay -q -D plughw:0,0 /tmp/mic-rec.wav");
    let wav = if std::path::Path::new("/opt/microduck-test/hwtest/bbno.wav").exists() {
        "/opt/microduck-test/hwtest/bbno.wav"
    } else {
        "./bbno.wav"
    };
    let detail;
    let mut audio_ok = setup && played;
    if audio_ok {
        match compare_audio(wav, "/tmp/mic-rec.wav") {
            Some((peak, similarity)) => {
                audio_ok = peak > 1e6 / 2f32.powi(31) && similarity >= 0.4;
                detail = format!(
                    "Recording peak {:.3e}, similarity vs original {:.0}% (PASS if: playback ok, peak >1e6, similarity >=40%)",
                    peak * 2f32.powi(31),
                    similarity * 100.0
                );
            }
            None => {
                audio_ok = false;
                detail = "Failed to parse the audio file".to_string();
            }
        }
    } else {
        detail = "Audio file missing or sound card setup failed (PASS if: playback ok, peak >1e6, similarity >=40%)".to_string();
    }
    report("Audio loopback (speaker + mic)", audio_ok, &detail, results);

    // Stop background loops ([x] keeps pkill from matching this shell);
    // LEDs off, fan stays on at full speed
    run(r#"pkill -f 'hwtest-led-loo[p]'; pkill -f 'hwtest-fan-loo[p]'; sleep 0.2; \
         for f in /sys/class/leds/face:*/brightness; do echo 0 > $f 2>/dev/null; done; \
         pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); \
         [ -n "$pwm" ] && echo 255 > $pwm"#);

    let done = &results[start..];
    let passed = done.iter().filter(|(_, v)| matches!(v, Verdict::Pass)).count();
    println!("Auto full check done: {}/{} items PASS", passed, done.len());
}

fn run_action(action: &Action, label: &str, results: &mut Vec<(String, Verdict)>) {
    println!("\n===== {label} =====");
    if !action.manual.is_empty() {
        println!("Note: {}", action.manual);
    }
    let mut args: Vec<String> = Vec::new();
    for param in action.params {
        match read_param(param) {
            Some(value) => args.push(value),
            None => {
                println!("Cancelled");
                return;
            }
        }
    }
    let mut failed = false;
    for tpl in action.cmds {
        let cmd = render(tpl, &args);
        println!("\n$ {cmd}");
        match Command::new("bash").arg("-c").arg(&cmd).status() {
            Ok(s) if s.success() => {}
            // Signal termination (e.g. Ctrl-C ending evtest) counts as pass
            Ok(s) => {
                if let Some(code) = s.code() {
                    println!("!! command exit code: {code}");
                    failed = true;
                }
            }
            Err(e) => {
                println!("!! cannot run command: {e}");
                failed = true;
            }
        }
    }
    println!("\nExpected: {}", action.expect);
    let verdict = if failed { Verdict::Fail } else { Verdict::Pass };
    println!("Result: {}\n", verdict.colored());
    results.push((label.to_string(), verdict));
}

// Returns None when the operator cancels the action.
fn read_param(param: &Param) -> Option<String> {
    match param {
        Param::Text { prompt: label, default } => loop {
            let hint = if default.is_empty() {
                format!("{label} (c to cancel): ")
            } else {
                format!("{label} [{default}] (c to cancel): ")
            };
            let input = prompt(&hint);
            match input.as_str() {
                "c" | "C" => return None,
                "" if !default.is_empty() => return Some(default.to_string()),
                "" => println!("Cannot be empty"),
                _ => return Some(input),
            }
        },
        Param::TextOptional { prompt: label, default } => {
            let hint = if default.is_empty() {
                format!("{label} (c to cancel): ")
            } else {
                format!("{label} [{default}] (c to cancel): ")
            };
            let input = prompt(&hint);
            match input.as_str() {
                "c" | "C" => None,
                "" => Some(default.to_string()),
                _ => Some(input),
            }
        }
        Param::Choice { prompt: label, options } => loop {
            println!("{label}:");
            for (i, (name, _)) in options.iter().enumerate() {
                println!("  {}) {}", i + 1, name);
            }
            match prompt("Select (c to cancel): ").as_str() {
                "c" | "C" => return None,
                input => match input.parse::<usize>() {
                    Ok(n) if n >= 1 && n <= options.len() => {
                        return Some(options[n - 1].1.to_string());
                    }
                    _ => println!("Invalid input, try again"),
                },
            }
        },
    }
}

fn render(tpl: &str, args: &[String]) -> String {
    let mut out = tpl.to_string();
    for (i, value) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), value);
    }
    out
}

fn prompt(msg: &str) -> String {
    print!("{msg}");
    io::stdout().flush().ok();
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => std::process::exit(0), // EOF (e.g. serial console dropped)
        Ok(_) => line.trim().to_string(),
    }
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(2))
                .map(|euid| euid == "0")
        })
        .unwrap_or(false)
}

// Returns (peak, similarity) of the recording vs the reference audio.
// Peak is in [-1, 1]. Similarity is the mean Pearson correlation of the
// per-band energy envelopes (acoustic paths destroy waveform phase, so
// time-domain correlation is meaningless over air).
fn compare_audio(reference: &str, recording: &str) -> Option<(f32, f32)> {
    let (mut refr, refr_rate) = read_wav_mono(reference)?;
    let (rec, rate) = read_wav_mono(recording)?;
    refr = resample(&refr, refr_rate, rate);
    const FRAME: usize = 2048;
    let nf = refr.len().min(rec.len()) / FRAME;
    if nf < 10 {
        return None;
    }
    let peak = rec.iter().fold(0f32, |m, v| m.max(v.abs()));
    // skip <500Hz: mains hum lives there
    let bin = |hz: usize| hz * FRAME / rate as usize;
    let bands = [(bin(500), bin(1000)), (bin(1000), bin(2000)), (bin(2000), bin(4000)), (bin(4000), bin(8000))];
    let envelopes = |x: &[f32]| -> Vec<Vec<f32>> {
        let hann: Vec<f32> = (0..FRAME)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FRAME as f32).cos())
            .collect();
        let mut out = vec![Vec::with_capacity(nf); bands.len()];
        for f in 0..nf {
            let frame: Vec<f32> = x[f * FRAME..(f + 1) * FRAME]
                .iter()
                .zip(&hann)
                .map(|(s, w)| s * w)
                .collect();
            let mag = fft_mag(&frame);
            for (b, (lo, hi)) in bands.iter().enumerate() {
                out[b].push(mag[*lo..*hi].iter().sum::<f32>() / (hi - lo) as f32);
            }
        }
        out
    };
    let env_ref = envelopes(&refr);
    let env_rec = envelopes(&rec);
    // under system load playback may start up to ~1s late; median over bands
    // is robust against a single noisy band
    let mut corrs: Vec<f32> = (0..bands.len())
        .map(|b| best_pearson(&env_ref[b], &env_rec[b], 8))
        .collect();
    corrs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let similarity = (corrs[corrs.len() / 2 - 1] + corrs[corrs.len() / 2]) / 2.0;
    Some((peak, similarity))
}

fn best_pearson(a: &[f32], b: &[f32], max_lag: isize) -> f32 {
    let mut best = 0f32;
    for lag in -max_lag..=max_lag {
        let (a0, b0) = if lag >= 0 { (0usize, lag as usize) } else { ((-lag) as usize, 0usize) };
        let len = (a.len() - a0).min(b.len() - b0);
        if len < 10 {
            continue;
        }
        let (x, y) = (&a[a0..a0 + len], &b[b0..b0 + len]);
        let mx = x.iter().sum::<f32>() / len as f32;
        let my = y.iter().sum::<f32>() / len as f32;
        let (mut num, mut dx, mut dy) = (0f32, 0f32, 0f32);
        for i in 0..len {
            let (u, v) = (x[i] - mx, y[i] - my);
            num += u * v;
            dx += u * u;
            dy += v * v;
        }
        if dx > 0.0 && dy > 0.0 {
            best = best.max(num / (dx * dy).sqrt());
        }
    }
    best
}

// In-place radix-2 FFT, returns magnitudes. Input length must be a power of 2.
fn fft_mag(frame: &[f32]) -> Vec<f32> {
    let n = frame.len();
    let mut re = frame.to_vec();
    let mut im = vec![0f32; n];
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let half = len / 2;
        let mut i = 0;
        while i < n {
            let (mut cwr, mut cwi) = (1f32, 0f32);
            for k in 0..half {
                let (ur, ui) = (re[i + k], im[i + k]);
                let vr = re[i + k + half] * cwr - im[i + k + half] * cwi;
                let vi = re[i + k + half] * cwi + im[i + k + half] * cwr;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + half] = ur - vr;
                im[i + k + half] = ui - vi;
                let nwr = cwr * wr - cwi * wi;
                cwi = cwr * wi + cwi * wr;
                cwr = nwr;
            }
            i += len;
        }
        len <<= 1;
    }
    re.iter().zip(&im).map(|(r, i)| (r * r + i * i).sqrt()).collect()
}

// Decodes a PCM WAV (16/32-bit int) to mono f32 in [-1, 1].
fn read_wav_mono(path: &str) -> Option<(Vec<f32>, u32)> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 12 || &data[0..4] != b"RIFF" {
        return None;
    }
    let mut pos = 12;
    let (mut channels, mut rate, mut bits) = (0u16, 0u32, 0u16);
    let mut pcm: Option<&[u8]> = None;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body_end = (pos + 8 + size).min(data.len());
        let body = &data[pos + 8..body_end];
        match id {
            b"fmt " => {
                channels = u16::from_le_bytes(body[2..4].try_into().ok()?);
                rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
                bits = u16::from_le_bytes(body[14..16].try_into().ok()?);
            }
            b"data" => pcm = Some(body),
            _ => {}
        }
        pos += 8 + size + (size & 1);
    }
    let pcm = pcm?;
    let channels = channels.max(1) as usize;
    let sample_bytes = match bits {
        16 => 2,
        32 => 4,
        _ => return None,
    };
    let frame_bytes = channels * sample_bytes;
    let frames = pcm.len() / frame_bytes;
    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut sum = 0f32;
        for c in 0..channels {
            let off = f * frame_bytes + c * sample_bytes;
            let v = if bits == 16 {
                i16::from_le_bytes(pcm[off..off + 2].try_into().ok()?) as f32 / 32768.0
            } else {
                i32::from_le_bytes(pcm[off..off + 4].try_into().ok()?) as f32 / 2f32.powi(31)
            };
            sum += v;
        }
        mono.push(sum / channels as f32);
    }
    Some((mono, rate))
}

fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (samples.len() as f64 / ratio) as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = (pos as usize).min(samples.len() - 1);
            let frac = (pos - idx as f64) as f32;
            samples[idx] + (samples[(idx + 1).min(samples.len() - 1)] - samples[idx]) * frac
        })
        .collect()
}
