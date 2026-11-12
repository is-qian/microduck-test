# WQ9201 WiFi / BT 定频测试（RSE）

MicroDuck（RK3566）搭载的 WiFi/BT 芯片为物奇 WQ9201（Wi-Fi 6 + BT 二合一，
SDIO + UART 接口）。本工具让芯片在指定频段 / 信道 / 制式下持续发射，
用于 RSE（辐射杂散）等认证测试，配合实验室接收机测量。

已在 microduck 上验证通过：内核 6.1.172-vendor-rk35xx，驱动版本 2.2.3.94。

命令参数定义参考：`testScripts/rockchip/RK3576-IO-Board/wifi-module`
（github.com/Lesords/testScripts）。

## 预制内容

FULL 镜像构建时本目录被复制到 `/opt/microduck-test/wifi-rf-test/`，
并由 `install.sh` 完成系统级安装，开箱即用：

- MP 厂测固件 → `/lib/firmware/`（与正常固件文件名不同，共存不冲突）
  - `wq9201_fw_wifi_1_1_dtest.bin`：WiFi 定频固件
  - `wq9201_fw_dtop_1_1_mp_sdio.bin`：dtop MP 固件
  - `wq9201_fw_bt_1_1_mp.bin`：蓝牙 MP 固件
- `ioctl_app` → `/usr/bin/ioctl_app`（aarch64 静态编译）
- 脚本：`/opt/microduck-test/wifi-rf-test/scripts/`
  - `load_mp_ko.sh`：卸载正常驱动，加载 MP 固件驱动
  - `enter_mp_mode.sh`：进入产测 verify 模式（含 RF 自校准）
  - `tx.sh`：定频 TX 持续发包（参数化）
  - `rx.sh`：定频 RX 配置与计数清零
- `reference/wq-commands.txt`：各频段 / 速率现成的原始命令块（厂家提供）

> 定频期间 wlan0 / hci0 完全不可用。RSE 在电波暗室中进行，务必确认
> 控制通道走有线（网口 / USB gadget / 串口），不要依赖 WiFi 远程连接。

## WiFi RSE 测试步骤

### 总体流程

```
接好控制链路 → load_mp_ko.sh → 放入暗室 → enter_mp_mode.sh（自校准）→
tx.sh 持续发射 → 实验室扫频测量 → tx.sh cancel → 换下一个配置 → … →
全部测完 → reboot 恢复
```

以下命令均在板子上执行，脚本路径记为
`SCRIPTS=/opt/microduck-test/wifi-rf-test/scripts`。

### 1. 加载 MP 驱动

```bash
sudo $SCRIPTS/load_mp_ko.sh
```

脚本会停掉 btattach、卸载 `wq_wlan`，再以 MP 固件参数重新加载
（`loadfw_only=1 fw_sys=5 force_reset=1`）。

### 2. 进入产测模式

**先把天线口接好（RSE 用整机天线即可；若传导测试则射频线接仪器）**，
然后执行：

```bash
sudo $SCRIPTS/enter_mp_mode.sh
```

其中 `dstart 15 1 0` 是 RF 自校准，**每次上电只执行一次**。整个测试
过程中只需执行一次本步骤，之后切换信道 / 制式不需要重新校准。

### 3. 持续发射（RSE 测量状态）

```bash
./tx.sh <channel> <mode> <rate> <bw> <power_dbm> [ant]
```

- channel：实际中心频率信道号（脚本自动按带宽换算，40M 减 2、80M 减 6）
- mode：0=b/g/a，2=n，4=ac，5=ax
- rate：b/g/a 直接写速率（1 2 5.5 11 6 9 12 18 24 36 48 54）；
  n=0-7；ac=0-9；ax=0-11
- bw：20/40/80（MHz）
- ant：1 或 2，默认 1

启动后芯片持续发包，**保持发射状态不要停**，实验室在此期间完成
该配置的辐射扫频测量。

### 4. 切换测试配置

**换一个信道 / 制式 / 功率前必须先停发**：

```bash
sudo $SCRIPTS/tx.sh cancel          # 停止发射
sudo $SCRIPTS/tx.sh 6 0 11 20 26    # 下一个配置
```

### 5. RSE 推荐测试配置（WiFi）

每个频段取低 / 中 / 高信道，每种制式用额定最大功率。功率值以下为
厂家命令样例中的参考值，实际以产品规格书 / 实验室要求为准：

| 频段 | 制式 | 信道（低/中/高） | tx.sh 示例（低信道） |
|---|---|---|---|
| 2.4G | 11b 11M | 1 / 6 / 11 | `tx.sh 1 0 11 20 26` |
| 2.4G | 11g 54M | 1 / 6 / 11 | `tx.sh 1 0 54 20 26` |
| 2.4G | 11n HT20 MCS7 | 1 / 6 / 11 | `tx.sh 1 2 7 20 20` |
| 2.4G | 11ax HE20 MCS11 | 1 / 6 / 11 | `tx.sh 1 5 11 20 17` |
| 5G | 11a 54M | 36 / 64 / 149 | `tx.sh 36 0 54 20 25` |
| 5G | 11n HT20 MCS7 | 36 / 64 / 149 | `tx.sh 36 2 7 20 19` |
| 5G | 11ax HE20 MCS11 | 36 / 64 / 149 | `tx.sh 36 5 11 20 16` |

说明：

- 信道按实际认证地区选取（如 CE 2.4G 高端用 CH13，5G 按认证频段选点）
- 双天线产品两根天线都要测：`tx.sh 36 5 11 20 16 2`（末位参数为天线）
- 若实验室要求更高占空比或单载波（CW），tx.sh 的 `tx_mpdu` 已是接近
  满载的持续发包；CW 单音命令需向物奇确认
- 更多组合的原始命令见 `reference/wq-commands.txt`

### 6. RX 灵敏度（如实验室需要）

```bash
sudo $SCRIPTS/rx.sh 7 20 1           # 配置信道/带宽/天线并清零计数
# 仪器按对应速率发固定数量的包
sudo ioctl_app wifi "rxratecnt 0 5 11"   # 查询收包计数（参数：模式 速率）
```

## 蓝牙 RSE 测试步骤

蓝牙定频无法用 Linux 命令完成，需要物奇厂测上位机（仅 Windows 版），
通过串口连接板子：

上位机的命令通道是板子的 Linux 串口控制台（默认 COM 口，1500000 8N1），
初始化时会在控制台上执行 `su` 等命令，**控制台必须已经是 root shell**。
板子上有两个会在 Windows 上枚举成 COM 口的控制台，注意区分：

- **UART2 调试口**（TP83 TX / TP84 RX / TP85 GND，3.3V，1500000）：
  root 自动登录，接上 USB 转 TTL 即可直接使用，推荐用这个口
- **USB-C gadget ACM 口**（ttyGS0）：走 `/bin/login`，需要用户名密码登录，
  上位机发 `su` 会被登录提示符吞掉，初始化阶段报 `Timeout(3s) for cmd: <su>`。
  判断方法：PC 拿到 gadget 子网地址（DHCP 网关即板子）即说明连的是 gadget 口。
  若只能用它，先 ssh 进板子（用户 `seeed`，地址为该网口的网关）给该控制台
  开 root 自动登录：

  ```sh
  sudo tee /etc/systemd/system/serial-getty@ttyGS0.service.d/autologin.conf <<'EOF'
  [Service]
  ExecStart=
  ExecStart=-/sbin/agetty --noissue --autologin root --noreset --noclear --keep-baud 115200,57600,38400,9600 ttyGS0 linux
  EOF
  sudo systemctl daemon-reload
  sudo systemctl restart serial-getty@ttyGS0
  ```

  用终端软件打开该 COM 口按回车，出现 `root@microduck:~#` 即生效；
  测试完成后删除该 drop-in 并 `daemon-reload` + restart 恢复。

1. 板子上先执行 `sudo $SCRIPTS/load_mp_ko.sh` 加载 MP 驱动
2. 断开串口占用，上位机选择对应串口，配置以下路径后初始化：

| 配置项 | 路径 |
|---|---|
| app | `/usr/bin/ioctl_app` |
| ko | `/lib/modules/$(uname -r)/updates/dkms/wq_wlan.ko` |
| PhyCfg | `/lib/firmware/wq9201_phy_1_1_4366_0102.bin` |
| FwDtop | `/lib/firmware/wq9201_fw_dtop_1_1_mp_sdio.bin` |
| FwWiFi | `/lib/firmware/wq9201_fw_wifi_1_1_dtest.bin` |
| FwBt | `/lib/firmware/wq9201_fw_bt_1_1_mp.bin` |

3. 在上位机中选择测试项目（BR/EDR 或 BLE、定频信道、发射功率），
   保持发射供实验室测量。RSE 典型配置：低 / 中 / 高信道
   （2402 / 2440 / 2480 MHz），BR/EDR 用 DH5 包型，BLE 用 1M/2M PHY，
   最大功率，分别测量

## 恢复正常模式

### 恢复操作

**直接重启板子**：

```bash
sudo reboot
```

> 不要试图 `rmmod` MP 驱动后重新加载正常固件——实测 MP 模式下 rmmod
> 会触发内核 oops（`wq_sdio_adma_process`），SDIO 主机挂死，之后即使
> `wifi_power` 上下电也无法重新 probe（error -110），只能重启恢复。
> 重启后系统自动加载正常固件，无需任何额外操作（MP 固件与正常固件
> 文件名不同，未覆盖原文件）。

### 恢复验证清单

重启后依次确认：

```bash
# 1. 驱动加载正常
lsmod | grep wq_wlan

# 2. wlan0 恢复
ip link show wlan0

# 3. WiFi 扫描 / 连接功能正常
sudo nmcli device wifi rescan ifname wlan0
nmcli device wifi list ifname wlan0
nmcli device wifi connect "<SSID>" password "<PASSWORD>" ifname wlan0

# 4. 蓝牙恢复（btattach 由系统服务自动拉起）
hciconfig hci0        # 应为 UP RUNNING，BD Address 非全零
bluetoothctl --timeout 10 -- scan on
```

## 常见问题

- `insmod: Invalid parameters`：ko 与内核 BTF 不匹配，执行
  `objcopy --remove-section=.BTF wq_wlan.ko` 后再加载
- TX 无输出：确认已执行 `enter_mp_mode.sh` 且 `dstart` 自校准成功；
  自校准必须在接好天线 / 仪器（或 50Ω 负载）后进行
- 改配置没生效：检查是否先执行了 `tx.sh cancel` 再下发新参数
