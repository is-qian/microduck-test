# alsa-config — ALSA 出厂配置(facemic 麦增益 + 扬声器路径)

统一维护整机的 ALSA 出厂状态,镜像阶段一次配置到位:

- **麦克风**:face-mic(card "facemic",MSM261DHP006 PDM 数字麦)链路无硬件
  增益(麦无寄存器,RK3566 PDM 控制器无增益位),由 `/etc/asound.conf` 的
  softvol `pcm.mic` 提供固定 +20dB(600 档、0.1dB/档,+20dB=档 400)
- **扬声器**:rk809 Playback Path **SPK_HP**——唯一同时给 HPL DAC 和外部
  NS4150B 功放供电的路径(dts 无 use-ext-amplifier,SPK 路径会把 HPL DAC
  断电),否则出厂开机无声。此项由 board-bundle 移入(2026-10-29),
  消除了两者先后安装时整文件互相覆盖 asound.state 的坑

## 文件

- `asound.conf` — softvol 设备定义(录音用 `-D mic`)
- `asound.state` — 完整出厂 state(扬声器 Playback Path/DAC + Face Mic
  Capture Volume 400),按卡名组织(state.facemic 等),不依赖卡编号

## 部署

**镜像阶段(自动,零板上操作)**:dist 的 `install.sh` 在镜像 chroot 里跑
一次,写入上述两个文件。开机 udev alsa-restore 按 state 里的 user 标记重建
softvol 控件并恢复 +20dB、应用 SPK_HP——首启即生效(2026-10-29 模拟首启
实测:开机后控件 400,400,第一次录音即带增益)。

**板上修复(手动,幂等)**:state 丢失或被覆盖后恢复(同 install.sh 的设备
阶段,立即生效):

```sh
sudo /opt/microduck-test/alsa-config/install.sh
```

## 使用

- 带增益录音:`arecord -D mic ...`(-D hw:facemic,0 直开不带走增益)
- 验证:`amixer -c facemic cget name='Face Mic Capture Volume'` 应为 400,400
- 调整增益:每 10 档 +1dB(上限 599=+39.9dB),>+24dB(档 440)近距离
  大声说话可能削波;改完 `sudo /usr/sbin/alsactl store`
- 录音必须 `-c 2`(PDM 控制器最少 2 通道)
- 注意:alsactl 在 `/usr/sbin`,ssh 非交互 shell 需全路径
