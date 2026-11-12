# MicroDuck 全功能 RE（Run-in）测试

`re-test.sh` 在板上同时压满所有外设模块，用于整机可靠性/老化测试。
FULL 镜像构建时随 test-tools 预制到 `/opt/microduck-test/re-test/`。
各模块独立后台进程、独立日志；硬件不在位的模块预检后自动跳过并记日志。
主循环每 10s 记录温度/频率/存活状态，每 60s 屏幕打印一行关键状态摘要；
Ctrl-C、超时或过热（默认 85℃）时统一清理恢复。

屏幕摘要示例：

```text
[14:19:21] STATUS 40C 1608MHz alive=9/9 | servo: 1:1863@30C 2:1854@29C bat:8.3V | fan:629Hz | wifi:94AP | nfc:nfcid=8E4BF51D | tof:14.76 fps
```

依次是 SoC 温度、CPU 频率、存活进程数、各舵机位置@温度与电池电压、
风扇转速、WiFi 扫到的 AP 数、NFC 卡 UID（无卡显示 no-tag）、ToF 实测帧率；
模块掉线会即时打印 `module xxx died`。

同时运行的模块：

| 模块 | 行为 |
|------|------|
| bt | 手柄进配对模式即可，自动扫描→配对→信任→连接，断线自动重连；每 60s 一次发现扫描（BT/WiFi 共存压力）；有 evtest 时抓取手柄事件 |
| wifi | 每 10s 触发一次全信道扫描，记录 BSS 数与连接状态 |
| cam | `/dev/video0` 1080p 连续取流（900 帧一轮循环）到 /dev/null |
| spk | 880Hz 正弦循环播放（RK809 → NS4150B） |
| mic | facemic PDM 双通道 16kHz 录音，录 MIC_REC 秒停 MIC_PAUSE 秒循环（默认 5s/5s，丢弃不落盘） |
| nfc | `nfc-test poll 5000` 循环轮询（卡需贴在天线上） |
| tof | 自动 stop tofd → `vl53l8cx-test` 8×8 测距，300 帧（20s）一轮循环并输出实测帧率；退出时恢复 tofd |
| imu | Face 小板 LSM6DSV16X 120Hz 出数，1Hz 采样记录原始寄存器 |
| led | face LED 全部点亮：AW2023 RGB 3 路 + AW21009 9 通道（退出时全部熄灭） |
| fan | pwmfan 全速，每 30s 统计 TACH 频率 |
| servo | 自动扫描总线（排除 GD32 200）→ 读取状态 → 全部回零位（2048）→ 各舵机 ±25° 缓慢往返（限速 SERVO_VEL，默认约 1.8s/25°）；每 10 个半周期记录各舵机位置/温度/电压/硬件错误 + GD32 `dxl sensors`（IMU/电池） |

## 使用方法

板上执行，必须 root；建议走 USB gadget ssh 或串口（WiFi 持续扫描会影响 WiFi ssh）：

```sh
sudo /opt/microduck-test/re-test/re-test.sh        # 镜像已预制
# 开发时手动部署：scp test-tools/re-test/re-test.{sh,conf} seeed@<board>:~/re-test/
```

所有可配置参数在脚本同目录的 `re-test.conf` 里（含注释），直接改文件即可
（预制路径 `/opt/microduck-test/` 为只读习惯，建议复制到 `~/re-test/` 再改，
脚本会在脚本同目录和 `$PWD` 下找 `re-test.conf`）；
环境变量优先级高于配置文件（`VAR=x sudo -E ./re-test.sh` 临时覆盖）：

| 参数 | 默认 | 说明 |
|------|------|------|
| `DURATION` | 0 | 测试时长（秒），0 = 跑到 Ctrl-C |
| `TEMP_MAX` | 85000 | 过热保护阈值（毫摄氏度） |
| `SKIP` | 空 | 关闭的模块，空格分隔 |
| `BT_MAC` | 空 | 手柄 MAC，空则关闭 bt 模块 |
| `SERVO_IDS` | auto | 舵机 ID 列表，auto = 自动扫描（排除 GD32 200） |
| `SERVO_BAUD` | 1000000 | 总线波特率 57600/1000000，自动切换含 GD32，退出恢复 1M |
| `SERVO_CENTER` / `SERVO_AMP` | 2048 / 284 | 零位与摆幅（284 = ±25°） |
| `SERVO_VEL` / `SERVO_PERIOD` | 10 / 3 | 摆动速度与半周期秒数 |
| `MIC_REC` / `MIC_PAUSE` | 5 / 5 | 录音/暂停秒数循环 |
| `FAN_PWM` | 255 | 风扇占空 0-255 |
| `DXL` / `TOF` / `NFC` | /opt/... | 测试工具路径 |

示例：

```sh
BT_MAC=AA:BB:CC:DD:EE:FF sudo -E /opt/microduck-test/re-test/re-test.sh   # 连接手柄（未配对会自动配对）
DURATION=28800 sudo /opt/microduck-test/re-test/re-test.sh                  # 跑 8 小时自动停
SKIP="bt nfc" sudo /opt/microduck-test/re-test/re-test.sh                   # 关闭部分模块
SERVO_BAUD=57600 sudo -E /opt/microduck-test/re-test/re-test.sh             # 总线切到 57600 测试，退出时自动恢复 1M
```

- 手柄进入配对模式即可，`bt` 模块会自动扫描→配对→信任→连接；`bluetoothctl devices` 可查已配对 MAC
- ToF 测试期间 tofd 被停止属预期，脚本退出会自动拉起
- 过压报警（err=128）的舵机扭矩保持不住时，先按
  `test/hardware-test.md` 24.2 节 `dxl fixvolt` 处理（本仓库预制到镜像后路径为 /opt/microduck-test/）
- `SERVO_BAUD` 支持 57600 / 1000000：与目标不同时会先把舵机（XL330 地址 8，
  EEPROM，需关扭矩）和 GD32（地址 8，存 flash）全部切过去再开测；
  退出时一律恢复 1000000（机器人软件总线默认 1M）

## 日志

`/tmp/re-test/<时间戳>/`：

- `<module>.log`：各模块原始输出（舵机日志含波特率切换结果、零位校验与周期性状态快照）
- `state/<module>`：各模块最新一行状态（屏幕摘要的数据源）
- `summary.csv`：时间、SoC 温度、CPU 频率、存活进程数（每 10s 一行）
- `main.log`：模块启停与异常事件

判读：全程无模块反复 restart、温度 < 85℃ 未触发保护、舵机位置跟随目标
且无 `hwerr` 非零、WiFi 扫描持续有 BSS、蓝牙不断连，即通过。
