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
    title: "变频检查",
    expect: "4 核；可用频率 408000~1800000 共 7 档；满载后 scaling_cur_freq 升至 1800000",
    manual: "",
    params: &[],
    cmds: &[
        "nproc",
        "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_available_frequencies",
        "for i in 1 2 3 4; do (timeout 3 yes > /dev/null &); done; sleep 1; \
         cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
    ],
};

// ------------------------------------------------------------------ 2. 内存

static MEM_INFO: Action = Action {
    title: "容量检查",
    expect: "Mem total 约 963Mi（1GB 颗粒）",
    manual: "",
    params: &[],
    cmds: &["free -h"],
};

// ------------------------------------------------------------------ 3. eMMC

static EMMC_INFO: Action = Action {
    title: "容量 / 型号",
    expect: "mmcblk0 容量约 58G（64GB 颗粒）",
    manual: "",
    params: &[],
    cmds: &[
        "lsblk -d -o NAME,SIZE,MODEL",
        "cat /sys/block/mmcblk0/device/name",
    ],
};

static EMMC_READ: Action = Action {
    title: "读取测速",
    expect: "直接读取速度不低于 80 MB/s",
    manual: "",
    params: &[Param::Text { prompt: "读取大小 (MB)", default: "256" }],
    cmds: &["dd if=/dev/mmcblk0 of=/dev/null bs=1M count={0} iflag=direct"],
};

static EMMC_MENU: Menu = Menu {
    title: "eMMC",
    items: &[Node::Action(&EMMC_INFO), Node::Action(&EMMC_READ)],
};

// ------------------------------------------------------------------ 4. PMIC

static PMIC: Action = Action {
    title: "电源轨核查",
    expect: "i2c-0 上 0x20(RK809)/0x41(RK860) 显示 UU；vdd_cpu 0.9~1.0V、vdd_logic 0.875V、\
             vdd_gpu/vdd_npu 0.9V、vcc_ddr 0.5V、vcc_1v8/vcca_1v8 1.8V、vccio_sd 3.3V、\
             cam_1v2 1.2V、cam_2v8 2.8V",
    manual: "",
    params: &[],
    cmds: &[
        "sudo i2cdetect -y -r 0",
        "for r in /sys/class/regulator/regulator.*/name; do n=$(cat $r); \
         case $n in vdd_cpu|vdd_logic|vdd_gpu|vdd_npu|vcc_ddr|vcc_1v8|vcca_1v8|vccio_sd|cam_1v2|cam_2v8) \
         echo \"$n $(cat ${r%name}microvolts)\";; esac; done",
    ],
};

// --------------------------------------------------------------- 5. 电源输入

static POWER_IN: Action = Action {
    title: "输入级电压",
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
    title: "RTC 时间一致性",
    expect: "hwclock 与 date 时间一致（误差 < 1s）",
    manual: "",
    params: &[],
    cmds: &["sudo hwclock --systohc && sudo hwclock --show && date"],
};

// ------------------------------------------------------------ 7. 温度 / 压测

static TEMP_NOW: Action = Action {
    title: "当前温度",
    expect: "输出为毫摄氏度（如 45000 = 45°C）",
    manual: "",
    params: &[],
    cmds: &["cat /sys/class/thermal/thermal_zone0/temp /sys/class/thermal/thermal_zone1/temp"],
};

static CPU_STRESS: Action = Action {
    title: "CPU 满载压测",
    expect: "stress-ng 报 successful run completed；满载 SoC 温度 ≤80°C，频率 ≥1.4GHz",
    manual: "",
    params: &[Param::Text { prompt: "压测时长 (秒)", default: "30" }],
    cmds: &[
        "cat /sys/class/thermal/thermal_zone0/temp",
        "sudo stress-ng --cpu 4 --timeout {0}",
        "cat /sys/class/thermal/thermal_zone0/temp /sys/class/thermal/thermal_zone1/temp; \
         cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq",
    ],
};

static THERMAL_MENU: Menu = Menu {
    title: "温度与压测",
    items: &[Node::Action(&TEMP_NOW), Node::Action(&CPU_STRESS)],
};

// -------------------------------------------------------------------- WiFi

static WIFI_STATUS: Action = Action {
    title: "连接状态",
    expect: "已连接时显示 SSID、信号优于 -70dBm、tx/rx bitrate 百兆级（HE）",
    manual: "",
    params: &[],
    cmds: &["sudo iw dev wlan0 link"],
};

static WIFI_SCAN: Action = Action {
    title: "扫描热点",
    expect: "能列出周边热点及信号强度",
    manual: "",
    params: &[],
    // "| cat": nmcli pages through less when stdout is a tty
    cmds: &["sudo nmcli -f SSID,CHAN,SIGNAL,SECURITY dev wifi list | cat"],
};

static WIFI_CONNECT: Action = Action {
    title: "连接热点",
    expect: "报 successfully activated；之后可用 ping 测试验证",
    manual: "",
    params: &[
        Param::Text { prompt: "SSID", default: "" },
        Param::Text { prompt: "密码", default: "" },
    ],
    cmds: &[r#"sudo nmcli dev wifi connect "{0}" password "{1}""#],
};

static WIFI_DISCONNECT: Action = Action {
    title: "断开连接",
    // device disconnect（而非 con down）会阻止 NM 自动重连，便于重复测试连接流程
    expect: "报 successfully disconnected；连接状态变为 disconnected",
    manual: "",
    params: &[],
    cmds: &["sudo nmcli dev disconnect wlan0"],
};

static WIFI_PING: Action = Action {
    title: "Ping 测试",
    expect: "0% 丢包",
    manual: "",
    params: &[Param::Text { prompt: "目标地址", default: "one.one.one.one" }],
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

// -------------------------------------------------------------------- 蓝牙

static BT_STATUS: Action = Action {
    title: "控制器状态",
    expect: "hci0 UP RUNNING，Bus: UART",
    manual: "",
    params: &[],
    cmds: &["sudo hciconfig -a"],
};

static BT_SCAN: Action = Action {
    title: "扫描设备",
    expect: "能发现周边 BLE/BT 设备",
    manual: "",
    params: &[Param::Text { prompt: "扫描时长 (秒)", default: "6" }],
    cmds: &["sudo bluetoothctl --timeout {0} scan on"],
};

static BT_PAD: Action = Action {
    title: "手柄连接与按键",
    expect: "选中手柄后配对连接成功，出现输入设备节点；按键/摇杆事件持续打印",
    manual: "先让手柄进入配对模式；选中设备后自动配对/连接并打印按键事件，\
             手柄断连或 Ctrl-C 结束",
    params: &[Param::Text { prompt: "扫描时长 (秒)", default: "8" }],
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
echo "扫描中（{0} 秒）..."
bluetoothctl --timeout {0} scan on > /dev/null 2>&1
mapfile -t devs < <(bluetoothctl devices | grep -iE 'controller|gamepad|xbox|8bitdo|joy-con|dualshock|dualsense')
if [ ${#devs[@]} -eq 0 ]; then
    echo "没有发现手柄类设备，列出全部已命名设备："
    mapfile -t devs < <(bluetoothctl devices | awk '{print $2, $3}' | grep -vE '^([0-9A-Fa-f:]{17}) ([0-9A-Fa-f]{2}-){5}')
fi
if [ ${#devs[@]} -eq 0 ]; then
    echo "没有发现任何设备，确认手柄处于配对模式后重试"
    exit 1
fi
i=1
for d in "${devs[@]}"; do echo "  $i) $d"; i=$((i+1)); done
read -rp "选择手柄编号: " n < /dev/tty
sel=${devs[$((n-1))]}
[ -z "$sel" ] && { echo "编号无效"; exit 1; }
MAC=$(echo "$sel" | awk '{print $2}' | grep -oE '([0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}')
echo "选中: $MAC"
bluetoothctl pair "$MAC" || true
bluetoothctl trust "$MAC" > /dev/null
if ! bluetoothctl connect "$MAC"; then
    echo "连接失败，清除旧配对后重试（请保持手柄处于配对模式）"
    bluetoothctl remove "$MAC" > /dev/null
    bluetoothctl --timeout {0} scan on > /dev/null 2>&1
    bluetoothctl pair "$MAC" || { echo "重新配对失败"; exit 1; }
    bluetoothctl trust "$MAC" > /dev/null
    bluetoothctl connect "$MAC" || { echo "连接失败"; exit 1; }
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
[ -z "$ev" ] && { echo "未找到手柄输入设备（HID 连接失败）"; exit 1; }
echo "输入设备: $ev ($(cat /sys/class/input/"$(basename "$ev")"/device/name))"
echo "按手柄按键查看事件，手柄断连或 Ctrl-C 结束"
evtest "$ev"
EOS"#;

static BT_MENU: Menu = Menu {
    title: "蓝牙",
    items: &[Node::Action(&BT_STATUS), Node::Action(&BT_SCAN), Node::Action(&BT_PAD)],
};

// --------------------------------------------------------------- USB-C OTG

static OTG: Action = Action {
    title: "OTG gadget 状态",
    expect: "state=configured（已连主机）；gadget 目录存在 rockchip",
    manual: "",
    params: &[],
    cmds: &[
        "cat /sys/class/udc/fcc00000.usb/state",
        "ls /sys/kernel/config/usb_gadget/",
    ],
};

// -------------------------------------------------------------------- 音频

static SPK_PLAY: Action = Action {
    title: "喇叭播放",
    expect: "aplay -l 有 card 0: rockchip,rk809-codec；播放无报错，喇叭可听到提示音",
    manual: "",
    params: &[
        Param::Text { prompt: "频率 (Hz)", default: "880" },
        Param::Text { prompt: "时长 (秒)", default: "2" },
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
    title: "PDM 麦克风录音",
    expect: "录满无报错；样本峰值非零（softvol +20dB 后约 1e8~1e9）",
    manual: "-D mic 走 softvol 固定增益，需先部署 alsa-config（完整镜像已内置）；\
             PDM 控制器最少 2 通道，必须 -c 2；没有 facemic 时检查 lsmod | grep dmic，\
             必要时 sudo modprobe snd-soc-dmic",
    params: &[Param::Text { prompt: "录音时长 (秒)", default: "3" }],
    cmds: &[
        "arecord -l",
        "sudo arecord -D mic -f S32_LE -r 16000 -c 2 -d {0} /tmp/mic-test.wav",
        r#"command -v python3 >/dev/null && python3 -c "import struct; d=open('/tmp/mic-test.wav','rb').read()[44:]; s=struct.unpack('<%di'%(len(d)//4), d); print('peak =', max(map(abs, s)))" || true"#,
    ],
};

static AUDIO_MENU: Menu = Menu {
    title: "音频",
    items: &[Node::Action(&SPK_PLAY), Node::Action(&MIC_REC), Node::Action(&AUDIO_LOOPBACK)],
};

// Runs via dispatch (custom flow + Rust-side comparison), not the generic runner.
static AUDIO_LOOPBACK: Action = Action {
    title: "音频回路（播放→录音→对比→回放）",
    expect: "",
    manual: "播放选中音频的同时录音，对比相似度后回放录音",
    params: &[
        Param::Choice {
            prompt: "播放音频",
            options: &[("测试音频 bbno.wav", ""), ("自定义路径", "custom")],
        },
        Param::TextOptional { prompt: "自定义音频路径（选测试音频直接回车）", default: "" },
        Param::Text { prompt: "播放/录音时长 (秒)", default: "10" },
    ],
    cmds: &[],
};

// ------------------------------------------------------------------ 摄像头

static CAM_TOPO: Action = Action {
    title: "拓扑检查",
    expect: "media 拓扑中存在 gc2093 或 gc2613 实体（未插的那个报 -EBUSY 属预期）",
    manual: "",
    params: &[],
    cmds: &["sudo media-ctl -d /dev/media0 -p | grep -E 'gc2093|gc2613'"],
};

static CAM_GRAB: Action = Action {
    title: "抓取裸帧",
    expect: "抓帧无丢失，文件大小 = 帧数 × 3110400 字节（NV12 1920x1080）",
    manual: "rkaiq_3A_server 运行中（pgrep rkaiq_3A_server）时画面亮度/色彩自动收敛",
    params: &[Param::Text { prompt: "帧数", default: "30" }],
    cmds: &[
        "sudo v4l2-ctl -d /dev/video0 --stream-mmap=3 --stream-count={0} --stream-to=/tmp/f.raw",
        "ls -l /tmp/f.raw",
    ],
};

static CAM_60FPS: Action = Action {
    title: "60fps 切换验证",
    expect: "120 帧完整（373248000 字节），实际帧率 ≥50fps；测完恢复 30fps",
    manual: "必须在无数据流时设置",
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
    title: "CSI 摄像头",
    items: &[Node::Action(&CAM_TOPO), Node::Action(&CAM_GRAB), Node::Action(&CAM_60FPS)],
};

// -------------------------------------------------------------- H.264 编码

static H264_ENC: Action = Action {
    title: "H.264 硬件编码",
    expect: "编码成功，输出平均帧率（约 30fps），生成非空 /tmp/test.h264",
    manual: "依赖摄像头抓帧生成的 /tmp/f.raw（NV12 1920x1080）",
    params: &[Param::Text { prompt: "编码帧数", default: "30" }],
    cmds: &[
        "sudo mpi_enc_test -i /tmp/f.raw -w 1920 -h 1080 -f 0 -t 7 -o /tmp/test.h264 -n {0}",
    ],
};

// ------------------------------------------------------- GPU/VPU/RGA2/NPU

static GPU_VPU: Action = Action {
    title: "设备节点检查",
    expect: "dri 有 card0/card1/renderD128/renderD129；/dev/mpp_service 存在；\
             devfreq 含 dmc/npu/gpu/rkvenc/rkvdec；dmesg 有 rga2 probe successfully",
    manual: "",
    params: &[],
    cmds: &[
        "ls /dev/dri/ /dev/mpp_service",
        "ls /sys/class/devfreq/",
        "dmesg | grep rga2",
    ],
};

// ----------------------------------------------------------- HWRNG/看门狗

static HWRNG: Action = Action {
    title: "HWRNG / 看门狗",
    expect: "读出 16 字节随机数；/dev/watchdog0 存在",
    manual: "",
    params: &[],
    cmds: &[
        "sudo dd if=/dev/hwrng bs=16 count=1 | od -t x1",
        "ls /dev/watchdog0",
    ],
};

// ------------------------------------------------------------------ EEPROM

static EEPROM: Action = Action {
    title: "Board ID EEPROM（24C32, I2C1-0x50）",
    expect: "0x50 显示 UU；sysfs 提供 4096 字节可读（出厂未烧录时全 0xFF）",
    manual: "",
    params: &[],
    cmds: &[
        "sudo i2cdetect -y -r 1",
        "sudo od -A x -t x1z /sys/bus/i2c/devices/1-0050/eeprom | head",
    ],
};

// ------------------------------------------------------------------- Qwiic

static QWIIC: Action = Action {
    title: "Qwiic 总线扫描（I2C3）",
    expect: "Face 小板 V1.0 连接时 0x20(AW21009)/0x45(AW2023) 显示 UU，0x6a(IMU) 可见",
    manual: "",
    params: &[],
    cmds: &["sudo i2cdetect -y 3"],
};

// ---------------------------------------------------------------- Face LED

static LED_LIST: Action = Action {
    title: "LED 列表",
    expect: "face:rgb:blue/green/red（AW2023）和 face:gr1/gr2/gb/green1/green2/cam（AW21009）共 12 个节点齐全",
    manual: "",
    params: &[],
    cmds: &["ls /sys/class/leds/ | grep -E 'face'"],
};

static LED_ON: Action = Action {
    title: "点亮 / 熄灭 LED",
    expect: "对应灯点亮，回车后熄灭",
    manual: "",
    params: &[Param::Text { prompt: "LED 名（见列表）", default: "face:rgb:red" }],
    cmds: &[
        "echo 255 | sudo tee /sys/class/leds/{0}/brightness > /dev/null; \
         read -p '{0} 应已点亮，确认后回车熄灭: ' -r; \
         echo 0 | sudo tee /sys/class/leds/{0}/brightness > /dev/null",
    ],
};

static LED_AW21009: Action = Action {
    title: "点亮 / 熄灭 AW21009 LED",
    expect: "对应灯点亮，回车后熄灭",
    manual: "face:gr1:red face:gr1:green face:green2:green face:green1:green face:gb:green face:gb:blue face:gr2:green face:gr2:red face:cam:red",
    params: &[Param::Text { prompt: "LED 名（见列表）", default: "face:cam:red" }],
    cmds: &[
        "echo 255 | sudo tee /sys/class/leds/{0}/brightness > /dev/null; \
         read -p '{0} 应已点亮，确认后回车熄灭: ' -r; \
         echo 0 | sudo tee /sys/class/leds/{0}/brightness > /dev/null",
    ],
};

static LED_MENU: Menu = Menu {
    title: "Face 小板 LED",
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
    title: "读取陀螺仪 + 加速度",
    expect: "静止时加速度模长约 1g（0.061 mg/LSB），陀螺仪零偏接近 0（4.375 mdps/LSB）",
    manual: "最后一步是测完关断，即使读数异常也应执行",
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
    title: "Face 小板 IMU",
    items: &[Node::Action(&IMU_ID), Node::Action(&IMU_READ)],
};

// --------------------------------------------------------------------- ToF

static TOF: Action = Action {
    title: "测距",
    expect: "is_alive 通过（device_id=0xF0）；init 成功，测距矩阵连续出数，\
             近处遮挡与背景距离对比清晰；末尾 data check 有效区域接近 N×N、\
             无 mostly invalid 帧",
    manual: "运行前后会自动 stop/start tofd，避免并发访问挂死传感器",
    params: &[
        Param::Text { prompt: "帧数", default: "20" },
        Param::Choice {
            prompt: "分辨率",
            options: &[("4x4", "4"), ("8x8", "8")],
        },
    ],
    cmds: &["sudo systemctl stop tofd; \
             sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 {0} {1}; \
             sudo systemctl start tofd"],
};

static TOF_FPS: Action = Action {
    title: "帧率验证（4x4 @ 60Hz）",
    expect: "frequency readback 为 60 Hz，实测约 58~60 fps，\
             data check 有效区域 16/16、无 mostly invalid 帧",
    manual: "60Hz 仅 4x4 支持（8x8 上限 15Hz）；运行前后会自动 stop/start tofd",
    params: &[Param::Text { prompt: "帧数", default: "600" }],
    cmds: &["sudo systemctl stop tofd; \
             sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 {0} 4 60; \
             sudo systemctl start tofd"],
};

// --------------------------------------------------------------------- NFC

static ANT_OPTS: &[(&str, &str)] = &[
    ("差分（双天线，默认）", ""),
    ("NFC1 单端（RFO1 = J30）", "NFC_ANT=1"),
    ("NFC2 单端（RFO2 = J31）", "NFC_ANT=2"),
];

static NFC_ID: Action = Action {
    title: "冒烟测试（IC_ID）",
    expect: "IC_ID = 0xA9，退出码 0",
    manual: "",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-id-test"],
};

static NFC_REGS: Action = Action {
    title: "寄存器核查",
    expect: "GENERAL[0x01]=0x0C（差分双 RFO）、REGULATOR[0x02]=0x5F、TX_DRIVER[0x03]=0x00",
    manual: "",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-test regs"],
};

static NFC_FIELD: Action = Action {
    title: "场强观测",
    expect: "空载 I 稳定在 75-80；卡片贴近天线后 I/Q 明显变化，移开恢复",
    manual: "1 次/秒持续输出，Ctrl-C 停止",
    params: &[],
    cmds: &["sudo /opt/microduck-test/nfc/nfc-test field"],
};

static NFC_POLL: Action = Action {
    title: "整卡轮询",
    expect: "标签贴在哪路，对应单端命令就能发现并报出卡 UID（E00223 开头）；\
             贴 J30 时 NFC_ANT=2 应报 no tag found，反之亦然",
    manual: "用 NFC_ANT 切单端可逐路验证天线，确认无串扰错位",
    params: &[
        Param::Choice { prompt: "天线", options: ANT_OPTS },
        Param::Text { prompt: "轮询时长 (ms)", default: "8000" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test poll {1}"],
};

static NFC_READ: Action = Action {
    title: "读块（ISO15693）",
    expect: "先打印卡 UID（E00223 开头），随后输出 4 字节块数据",
    manual: "",
    params: &[
        Param::Choice { prompt: "天线", options: ANT_OPTS },
        Param::Text { prompt: "块号", default: "0" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv read {1}"],
};

static NFC_WRITE_VERIFY: Action = Action {
    title: "写块（写回校验，非破坏性）",
    expect: "输出 WRITE PASS（写后回读一致），退出码 0",
    manual: "读出原数据写回并校验；若报协议错误且读正常，多为该块已被卡片锁定",
    params: &[
        Param::Choice { prompt: "天线", options: ANT_OPTS },
        Param::Text { prompt: "块号", default: "1" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv write {1}"],
};

static NFC_WRITE_DATA: Action = Action {
    title: "写块（指定数据）",
    expect: "输出 WRITE PASS（写后回读一致），退出码 0",
    manual: "破坏性写入，确认块用途后再执行",
    params: &[
        Param::Choice { prompt: "天线", options: ANT_OPTS },
        Param::Text { prompt: "块号", default: "0" },
        Param::Text { prompt: "数据（8 位 hex，如 aabbccdd）", default: "" },
    ],
    cmds: &["sudo {0} /opt/microduck-test/nfc/nfc-test nfcv write {1} {2}"],
};

static NFC_WRITE_ESL: Action = Action {
    title: "写墨水屏（ESL 电子标签）",
    expect: "上传 4 个位面后打印 refresh done；墨水屏显示对应图片",
    manual: "FMSC ESL 标签（1.54 寸六色 240x240）需全程贴在天线上直到 refresh done；\
             自定义图片须为 240x240 的 PNG/JPG/BMP/PPM，建议先按六色量化",
    params: &[
        Param::Choice {
            prompt: "图片",
            options: &[("内置测试图（六色彩条）", ""), ("自定义图片路径", "custom")],
        },
        Param::TextOptional { prompt: "自定义图片路径（选内置测试图直接回车）", default: "" },
        Param::Choice { prompt: "天线", options: ANT_OPTS },
    ],
    cmds: &["sudo {2} /opt/microduck-test/nfc/nfc-test write {1}"],
};

static NFC_MENU: Menu = Menu {
    title: "NFC（ST25R100）",
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

// --------------------------------------------------------------------- 风扇

static FAN_AUTO: Action = Action {
    title: "PWM / TACH 自动测试",
    expect: "全速 TACH 约 600Hz，半速约 315Hz，停止后 0 脉冲，调速基本线性",
    manual: "需连接风扇实物",
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
    title: "手动设定转速",
    expect: "pwm 写读一致，风扇转速随之变化",
    manual: "",
    params: &[Param::Choice {
        prompt: "占空比",
        options: &[("全速 (255)", "255"), ("半速 (128)", "128"), ("停止 (0)", "0")],
    }],
    cmds: &[
        "pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); \
         [ -n \"$pwm\" ] || { echo 'no pwmfan hwmon found'; exit 1; }; \
         echo 1 | sudo tee ${pwm}_enable > /dev/null; \
         echo {0} | sudo tee $pwm > /dev/null; cat $pwm",
    ],
};

static FAN_MENU: Menu = Menu {
    title: "风扇",
    items: &[Node::Action(&FAN_AUTO), Node::Action(&FAN_SET)],
};

// ------------------------------------------------------------------ UART3

static UART3: Action = Action {
    title: "端口检查",
    expect: "/dev/ttyS3 存在且可正常配置",
    manual: "",
    params: &[],
    cmds: &["ls /dev/ttyS3 && stty -F /dev/ttyS3 115200"],
};

static DXL_SCAN: Action = Action {
    title: "总线扫描",
    expect: "列出应答设备的 (ID, model)：GD32 电源板 (200, 1200)，舵机 (1, 1200)，可能有第二舵机 (2, 1200)；电源板伪装成 XL330，靠 ID 200 区分",
    manual: "扫 1 Mbps，约 1 分钟",
    params: &[],
    cmds: &["sudo /opt/microduck-test/dxl/dxl scan 1000000"],
};

static DXL_GD32: Action = Action {
    title: "GD32 电源板",
    expect: "ping 10/10、model 1200（伪装 XL330）、fw 7、err=0；imu_online=true，电压 5~20 V",
    manual: "ID 200；err=128 只是硬件告警标志，err=0 才是无告警",
    params: &[],
    cmds: &[
        "sudo /opt/microduck-test/dxl/dxl ping 200",
        "sudo /opt/microduck-test/dxl/dxl sensors",
    ],
};

static DXL_SERVO_PING: Action = Action {
    title: "舵机 ping / 状态",
    expect: "ping 10/10、model 1200 (XL330)；电压读数为 0.1 V/计数",
    manual: "err=128 表示有硬件告警，原因看 hw_err（bit0=电压告警，8.3 V 母线下属预期，不影响运转）",
    params: &[Param::Text { prompt: "舵机 ID", default: "1" }],
    cmds: &[
        "sudo /opt/microduck-test/dxl/dxl ping {0}",
        "sudo /opt/microduck-test/dxl/dxl read {0} 70 1",
        "sudo /opt/microduck-test/dxl/dxl read {0} 144 2",
    ],
};

static DXL_SERVO_MOVE: Action = Action {
    title: "舵机转动测试",
    expect: "舵机以零位为中心按设定摆幅往返，结束回零位后自动卸力，0 errors",
    manual: "观察舵机实际转动；不转或抖动判 FAIL",
    params: &[
        Param::Text { prompt: "舵机 ID", default: "1" },
        Param::Text { prompt: "时长 (秒)", default: "10" },
        Param::Text { prompt: "摆幅 (±度)", default: "15" },
    ],
    cmds: &["sudo /opt/microduck-test/dxl/dxl move {0} {1} {2}"],
};

static DXL_SERVO_FIXVOLT: Action = Action {
    title: "舵机过压过温修复",
    expect: "Shutdown bit0/bit1 清除并重启，复查 hw_err",
    manual: "仅当舵机因过压/过热保护锁死（Shutdown bit0/bit1 置位）时使用；过热修复须等舵机冷却，否则会再次跳闸；8.3 V 母线下 hw_err bit0 仍会置位，属预期",
    params: &[Param::Text { prompt: "舵机 ID", default: "1" }],
    cmds: &["sudo /opt/microduck-test/dxl/dxl fixvolt {0}"],
};

static DXL_SERVO_SETUP: Action = Action {
    title: "配置新舵机",
    expect: "出厂舵机 (ID 1 @ 57600) 写入目标 ID 和 1 Mbps 波特率，最后在新配置下 ping 通",
    manual: "总线上一次只能接一个新舵机；检测到多个 57600 设备会拒绝执行",
    params: &[Param::Text { prompt: "目标 ID", default: "1" }],
    cmds: &["sudo /opt/microduck-test/dxl/dxl setup {0}"],
};

static DXL_MENU: Menu = Menu {
    title: "Dynamixel 总线 (UART3)",
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

// ------------------------------------------------------------ Recovery 键

static RECOVERY_KEY: Action = Action {
    title: "Recovery 键",
    expect: "松开 SW2 时 raw=1023，按住时读数显著下降",
    manual: "下载模式需单独验证：u-boot 阶段按住 SW2 重启，PC 端 rkdeveloptool ld 应显示 Loader 设备",
    params: &[],
    cmds: &[
        "cat /sys/bus/iio/devices/iio:device0/in_voltage0_raw; \
         read -p '按住 SW2 不放并回车: ' -r; \
         cat /sys/bus/iio/devices/iio:device0/in_voltage0_raw",
    ],
};

// ----------------------------------------------------------------- Maskrom

static MASKROM: Action = Action {
    title: "Maskrom 测试点",
    expect: "PC 端能识别 Maskrom 设备并正常烧录",
    manual: "需断电操作：断电后短接 TP64（eMMC_D0）到 TP4（GND），上电进入 maskrom，\
             PC 端 rkdeveloptool ld 应显示 Maskrom 设备",
    params: &[],
    cmds: &[],
};

// ------------------------------------------------------------------- HW_ID

static HW_ID: Action = Action {
    title: "HW_ID（SARADC ch1）",
    expect: "raw=1023，对应 Version 1（Rup 10K / Rdown DNP）",
    manual: "",
    params: &[],
    cmds: &["cat /sys/bus/iio/devices/iio:device0/in_voltage1_raw"],
};

// ----------------------------------------------------------- DDR/CPU 压力

static MEMTESTER: Action = Action {
    title: "memtester（单遍全项目）",
    expect: "Stuck Address / Random Value / XOR / Walking Ones 等全部 ok",
    manual: "",
    params: &[Param::Text { prompt: "测试大小", default: "128M" }],
    cmds: &["sudo memtester {0} 1"],
};

static DDR_FREQ: Action = Action {
    title: "DDR 变频",
    expect: "输出 success change to ... Hz；cur_freq 与目标一致",
    manual: "测完请执行“恢复 DDR 变频”",
    params: &[Param::Choice {
        prompt: "目标频率",
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
    title: "恢复 DDR 变频（dmc_ondemand）",
    expect: "governor 恢复 dmc_ondemand",
    manual: "",
    params: &[],
    cmds: &["echo dmc_ondemand | sudo tee /sys/class/devfreq/dmc/governor"],
};

static CPU_FREQ_STRESS: Action = Action {
    title: "CPU 变频压力（约 100 秒）",
    expect: "每档输出 successfully changed；stress-ng 报 successful run completed；\
             测完 governor 恢复 ondemand",
    manual: "负载下逐档变频",
    params: &[],
    cmds: &[
        "sudo stress-ng --cpu 4 --timeout 60 & sleep 2; \
         for f in 408000 816000 1416000 1800000 600000 1104000 1608000; do \
         sudo bash /opt/microduck-test/rockchip-test/cpu/cpu_freq_scaling.sh $f; sleep 5; done; \
         echo ondemand | sudo tee /sys/devices/system/cpu/cpufreq/policy0/scaling_governor; wait",
    ],
};

static STRESSAPP: Action = Action {
    title: "stressapptest（DDR 压力）",
    expect: "Status: PASS，0 错误",
    manual: "量产建议 24h",
    params: &[Param::Text { prompt: "压测时长 (秒)", default: "300" }],
    cmds: &[
        "sudo mkdir -p /userdata/rockchip-test && \
         sudo stressapptest -s {0} -i 4 -C 4 -W --stop_on_errors -M 128 \
         -l /userdata/rockchip-test/stressapptest.log",
    ],
};

static STRESS_MENU: Menu = Menu {
    title: "DDR / CPU 压力",
    items: &[
        Node::Action(&MEMTESTER),
        Node::Action(&DDR_FREQ),
        Node::Action(&DDR_FREQ_RESTORE),
        Node::Action(&CPU_FREQ_STRESS),
        Node::Action(&STRESSAPP),
    ],
};

// ---------------------------------------------------------------- 自动全检

// Runs via run_auto_check (quiet, auto-judged), not the generic runner.
static AUTO_CHECK: Action = Action {
    title: "自动全检",
    expect: "",
    manual: "全程自动判定，无需输入；请预先将两张 NFC 标签分别贴到 J30 / J31 天线上。\n\
             期间全部 LED 一起由暗到亮再到暗渐变、风扇由低到高循环变速，结束后 LED 熄灭、风扇保持全速运转。",
    params: &[],
    cmds: &[],
};

// ----------------------------------------------------------------- 主菜单

static MAIN_MENU: Menu = Menu {
    title: "MicroDuck D4 硬件测试",
    items: &[
        Node::Menu(&Menu { title: "SoC / CPU 变频", items: &[Node::Action(&SOC_FREQ)] }),
        Node::Menu(&Menu { title: "内存", items: &[Node::Action(&MEM_INFO)] }),
        Node::Menu(&EMMC_MENU),
        Node::Menu(&Menu { title: "PMIC 电源轨", items: &[Node::Action(&PMIC)] }),
        Node::Menu(&Menu { title: "电源输入级", items: &[Node::Action(&POWER_IN)] }),
        Node::Menu(&Menu { title: "RTC", items: &[Node::Action(&RTC)] }),
        Node::Menu(&THERMAL_MENU),
        Node::Menu(&WIFI_MENU),
        Node::Menu(&BT_MENU),
        Node::Menu(&Menu { title: "USB-C OTG", items: &[Node::Action(&OTG)] }),
        Node::Menu(&AUDIO_MENU),
        Node::Menu(&CAMERA_MENU),
        Node::Menu(&Menu { title: "H.264 硬件编码", items: &[Node::Action(&H264_ENC)] }),
        Node::Menu(&Menu { title: "GPU / VPU / RGA2 / NPU", items: &[Node::Action(&GPU_VPU)] }),
        Node::Menu(&Menu { title: "HWRNG / 看门狗", items: &[Node::Action(&HWRNG)] }),
        Node::Menu(&Menu { title: "Board ID EEPROM", items: &[Node::Action(&EEPROM)] }),
        Node::Menu(&Menu { title: "Qwiic（I2C3）", items: &[Node::Action(&QWIIC)] }),
        Node::Menu(&LED_MENU),
        Node::Menu(&IMU_MENU),
        Node::Menu(&Menu { title: "Face 小板 ToF", items: &[Node::Action(&TOF), Node::Action(&TOF_FPS)] }),
        Node::Menu(&NFC_MENU),
        Node::Menu(&FAN_MENU),
        Node::Menu(&DXL_MENU),
        Node::Menu(&Menu { title: "Recovery 键 / 下载模式", items: &[Node::Action(&RECOVERY_KEY)] }),
        Node::Menu(&Menu { title: "Maskrom 测试点", items: &[Node::Action(&MASKROM)] }),
        Node::Menu(&Menu { title: "HW_ID", items: &[Node::Action(&HW_ID)] }),
        Node::Menu(&STRESS_MENU),
        Node::Menu(&Menu { title: "自动全检", items: &[Node::Action(&AUTO_CHECK)] }),
    ],
};

// ------------------------------------------------------------------ 运行时

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
        println!("!! 当前不是 root，部分项目需要 sudo；建议以 sudo hwtest 运行");
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
    println!("\n===== 测试总结 =====");
    let mut report = String::new();
    for (label, verdict) in &results {
        println!("{:<48} {}", label, verdict.colored());
        let line = format!("{:<48} {}", label, verdict.label());
        report.push_str(&line);
        report.push('\n');
    }
    let path = "/tmp/hwtest-report.txt";
    match std::fs::write(path, &report) {
        Ok(()) => println!("\n报告已写入 {path}"),
        Err(e) => eprintln!("无法写入 {path}: {e}"),
    }
}

fn usage(prog: &str) -> ! {
    eprintln!("usage: {prog}         交互式菜单");
    eprintln!("       {prog} --list  打印测试项目树");
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
            "编号选择，q 退出"
        } else {
            "编号选择，b 返回上级，m 主菜单，q 退出"
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
                _ => println!("输入无效，请重试"),
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
    println!("说明: {}", action.manual);
    let mut args: Vec<String> = Vec::new();
    for param in action.params {
        match read_param(param) {
            Some(value) => args.push(value),
            None => {
                println!("已取消");
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
    println!("播放并录音 {dur}s...");
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
                "录音峰值 {:.3e}，与原音频相似度 {:.0}%（参考：相似度 ≥40% 为好）",
                peak * 2f32.powi(31),
                similarity * 100.0
            );
        }
        None => {
            ok = false;
            println!("音频文件解析失败");
        }
    }
    println!("回放录音...");
    run("aplay -q -D plughw:0,0 /tmp/mic-rec.wav");
    // 相似度自动判定仅供参考，最终以人耳听到的回放为准
    println!("相似度自动判定: {}", if ok { Verdict::Pass.colored() } else { Verdict::Fail.colored() });
    let verdict = loop {
        match prompt("判定 [P]ass / [F]ail / [S]kip (默认 s): ").to_ascii_lowercase().as_str() {
            "p" | "pass" => break Verdict::Pass,
            "f" | "fail" => break Verdict::Fail,
            "" | "s" | "skip" => break Verdict::Skip,
            _ => println!("请输入 p / f / s"),
        }
    };
    println!("结果: {}\n", verdict.colored());
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

    println!("\n===== 自动全检 =====");
    println!("说明: {}", AUTO_CHECK.manual);

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
        results.push((format!("自动全检 / {name}"), verdict));
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
    report("WiFi 扫描", ok, &format!("发现 {} 个热点（PASS 条件：≥1）", out.trim()), results);

    // Bluetooth scan
    let (up, _) = run("sudo hciconfig hci0 2>/dev/null | grep -q 'UP RUNNING'");
    let (_, out) = run("sudo bluetoothctl --timeout 5 scan on 2>/dev/null | \
         grep -oiE 'device ([0-9a-f]{2}:){5}[0-9a-f]{2}' | sort -u | wc -l");
    report(
        "蓝牙扫描",
        up,
        &format!(
            "控制器 {}，发现 {} 台设备（PASS 条件：控制器 UP RUNNING）",
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
            "IMU 读数",
            true,
            &format!("WHO_AM_I={id}（PASS 条件：==0x70）\n    陀螺仪+加速度: {}", data.trim()),
            results,
        );
    } else {
        report(
            "IMU 读数",
            false,
            &format!("WHO_AM_I={id}（期望 0x70；读不到多为 Face 小板未接或 I2C3 异常）"),
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
        "摄像头拍照",
        ok,
        &format!("/tmp/photo.jpg {} 字节（PASS 条件：抓帧成功且照片非空）", size.trim()),
        results,
    );

    // ToF ranging
    let (ok, out) = run("sudo systemctl stop tofd; \
         sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 5 4 2>&1; \
         rc=$?; sudo systemctl start tofd; exit $rc");
    report("ToF 测距", ok, &format!("{}\n    （PASS 条件：init ok 且退出码 0）", out.trim()), results);

    // GD32 power board over Dynamixel: dxl sensors exits 0 only when the
    // GD32 answers with err=0 and the IMU is online (exit 3 = IMU offline)
    let (ok, out) = run("sudo /opt/microduck-test/dxl/dxl sensors 2>&1");
    report(
        "GD32 电源板 (Dynamixel)",
        ok,
        &format!("{}\n    （PASS 条件：ID 200 应答、err=0、imu_online=true）", out.trim()),
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
        "EEPROM 读写恢复",
        ok,
        &format!("{}\n    （PASS 条件：写入回读一致，且原数据恢复一致）", out.trim()),
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
        "NFC 轮询（双天线）",
        nfc_ok,
        &format!("{}\n    （PASS 条件：两路天线都读到卡 UID）", lines.join("\n    ")),
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
                    "录音峰值 {:.3e}，与原音频相似度 {:.0}%（PASS 条件：播放成功、峰值 >1e6、相似度 ≥40%）",
                    peak * 2f32.powi(31),
                    similarity * 100.0
                );
            }
            None => {
                audio_ok = false;
                detail = "音频文件解析失败".to_string();
            }
        }
    } else {
        detail = "音频文件缺失或声卡设置失败（PASS 条件：播放成功、峰值 >1e6、相似度 ≥40%）".to_string();
    }
    report("音频回路（喇叭 + 麦克）", audio_ok, &detail, results);

    // Stop background loops ([x] keeps pkill from matching this shell);
    // LEDs off, fan stays on at full speed
    run(r#"pkill -f 'hwtest-led-loo[p]'; pkill -f 'hwtest-fan-loo[p]'; sleep 0.2; \
         for f in /sys/class/leds/face:*/brightness; do echo 0 > $f 2>/dev/null; done; \
         pwm=$(ls -d /sys/class/hwmon/hwmon*/pwm1 2>/dev/null | head -1); \
         [ -n "$pwm" ] && echo 255 > $pwm"#);

    let done = &results[start..];
    let passed = done.iter().filter(|(_, v)| matches!(v, Verdict::Pass)).count();
    println!("自动全检完成: {}/{} 项 PASS", passed, done.len());
}

fn run_action(action: &Action, label: &str, results: &mut Vec<(String, Verdict)>) {
    println!("\n===== {label} =====");
    if !action.manual.is_empty() {
        println!("说明: {}", action.manual);
    }
    let mut args: Vec<String> = Vec::new();
    for param in action.params {
        match read_param(param) {
            Some(value) => args.push(value),
            None => {
                println!("已取消");
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
                    println!("!! 命令退出码: {code}");
                    failed = true;
                }
            }
            Err(e) => {
                println!("!! 无法执行命令: {e}");
                failed = true;
            }
        }
    }
    println!("\n预期: {}", action.expect);
    let verdict = if failed { Verdict::Fail } else { Verdict::Pass };
    println!("结果: {}\n", verdict.colored());
    results.push((label.to_string(), verdict));
}

// Returns None when the operator cancels the action.
fn read_param(param: &Param) -> Option<String> {
    match param {
        Param::Text { prompt: label, default } => loop {
            let hint = if default.is_empty() {
                format!("{label} (c 取消): ")
            } else {
                format!("{label} [{default}] (c 取消): ")
            };
            let input = prompt(&hint);
            match input.as_str() {
                "c" | "C" => return None,
                "" if !default.is_empty() => return Some(default.to_string()),
                "" => println!("不能为空"),
                _ => return Some(input),
            }
        },
        Param::TextOptional { prompt: label, default } => {
            let hint = if default.is_empty() {
                format!("{label} (c 取消): ")
            } else {
                format!("{label} [{default}] (c 取消): ")
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
            match prompt("选择 (c 取消): ").as_str() {
                "c" | "C" => return None,
                input => match input.parse::<usize>() {
                    Ok(n) if n >= 1 && n <= options.len() => {
                        return Some(options[n - 1].1.to_string());
                    }
                    _ => println!("输入无效，请重试"),
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
