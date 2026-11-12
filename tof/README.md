# Face 小板 ToF（VL53L8CX）SPI 测距测试

- 传感器：ST VL53L8CX（Face 小板 sheet 03，U1）
- 接口：SPI3 M0（VCCIO6，1.8V 域），CS=SPI3_CS0_1V8_M0（GPIO4_A6）
- 中断/同步：TOF_INT=GPIO4_B6、TOF_SYNC=GPIO4_B7（本测试未使用，采用轮询）
- 内核节点：`rk3566-microduck.dtsi` 中 `&spi3` 以 spidev 方式启用，用户态跑 ST ULD

## 源码

`vl53l8cx_api.c` / `vl53l8cx_buffers.h` 等为 ST 官方 ULD（取自 stm32duino/VL53L8CX，
与 STSW-IMG040 一致），`platform_linux.c` 为 spidev 平台层：

- SPI mode 3，MSB first，写地址 bit15=1、读地址 bit15=0
- 单次传输按 2048 字节分块（spidev 默认 bufsiz 4096）

## 构建与运行

FULL 镜像构建时随 test-tools 仓库预装到 `/opt/microduck-test/tof/`
并在 chroot 中完成编译（`install.sh`），板上直接运行：

```sh
sudo /opt/microduck-test/tof/vl53l8cx-test [spidev] [帧数] [分辨率4|8] [帧率Hz]
# 默认：/dev/spidev3.0 20帧 8x8（8x8 默认 15Hz，4x4 默认 60Hz）
sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 10 4
```

输出每帧 N×N 的 `distance_mm` 矩阵（帧数 >20 时只打印最后一帧），
末尾输出实测 fps 与数据校验（有效区域数统计，status 5/6/9 为有效）。
手动重新构建：`make -C /opt/microduck-test/tof`。

注意：运行前必须 `sudo systemctl stop tofd`，两个进程同时访问
/dev/spidev3.0 会互相干扰并可能挂死传感器（只能断电恢复）。
