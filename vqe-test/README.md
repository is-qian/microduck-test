# vqe-test — facemic PDM 麦 VQE(ANR+AGC)处理链

SDK(`librkaudio_vqe`)在 microduck 板(RK3566)上的可运行封装。链路无硬件增益
(见 `../facemic-gain/`),VQE 提供 ANR 降噪 + AGC 自动增益的动态语音前端,
适合喂 ASR/对讲;与 softvol 固定增益二选一,VQE 输入必须取原始流
(`-D hw:facemic,0`),不要叠 softvol。

仓库只放源码(Rust,零外部 crate);`install.sh <dir>` 由 `../build-dist.sh`/CI
调用,交叉编译并把自包含 payload(bin + so + 配置 + wrapper)装进 dist,板上
位于 `/opt/microduck-test/vqe-test/`,解包即用。闭源库 vendor 在 `vendor/`
(与 `nfc/rfal` 同样的做法),CI 无需 SDK;需要换库版本时 `SDK=` 指向完整
SDK 构建即可。

构建说明:与 hwtest 的 musl 静态不同,本工具用 `aarch64-unknown-linux-gnu`
动态链接——闭源 `librkaudio_vqe.so` 是 glibc 构建,musl 静态二进制无法加载它。
Rust 生成的 glibc 符号需求实测最高 2.34(板 2.36,兼容)。

## 已知坑(2026-10-29 验证结论)

- `librkaudio_common.so` 读 `/proc/device-tree/compatible` 做芯片白名单,只认
  `rockchip,rv1126b`——rk3566 被拒。wrapper 用 `mount --bind` 临时伪造,
  退出自动恢复;正式产品化前应向 RK/Seeed 确认支持 rk3566 的库版本
- **必须走 `rkaudio_preprocess_init_by_conf(json)`**:手写 `RKAUDIOParam` 的
  `init()` 在这版库上静默输出全零(程序保留 flags 数字参数作对照调试)
- 帧参数:256 样点/帧@16k(16ms),2ch interleaved,`preprocess_short` 第 4
  参传 512(shorts);SDK asr demo 源码传字节,与官方 PDF 文档矛盾,以文档为准。
  返回值是字节数(每帧 512 字节 = 256 shorts,板上实测)
- 输入要求:16k mono S16 wav;程序内部复制成 2ch 喂库

## 板上使用(/opt/microduck-test/vqe-test/)

### 实时(推荐)

```sh
sudo /opt/microduck-test/vqe-test/vqe-live /tmp/raw.wav /tmp/out.wav [秒数,默认10]
```

facemic 原始流采集(hw:facemic,不走 softvol)→ 逐帧 VQE → 落盘两份 mono
S16(原始对照 + 处理后),运行中每秒打印 in/out 实时电平。语音服务集成时,
`vqe_live.c` 的 stdin 循环就是参考模板(SDK `app/lvgl_demo/rk_demo/asr/audio.c`
是带唤醒词的完整版)。

### 离线

```sh
# 录原始流(S32 2ch)并转 16k mono S16:
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

预期:纯底噪段输出 rms 下降(ANR),语音/瞬态段上升(AGC)。参考(环境音):
rms 82.5 → 260.5(+10dB);纯底噪:72.3 → 15.2(-13.5dB)。

## config_mic.json

由 SDK `rkaudio_algorithms/conf/config_aivqe.json` 生成:所有模块
`status` 置 disable,仅保留 `bf`/`agc`/`anr` enable(参数模板默认)。
AEC/wakeup/AINR 未启用(后者需 NPU rknn 模型)。
