# Face Board ToF (VL53L8CX) SPI Ranging Test

- Sensor: ST VL53L8CX (face board sheet 03, U1)
- Interface: SPI3 M0 (VCCIO6, 1.8V domain), CS=SPI3_CS0_1V8_M0 (GPIO4_A6)
- Interrupt/sync: TOF_INT=GPIO4_B6, TOF_SYNC=GPIO4_B7 (unused by this
  test — polling instead)
- Kernel node: `&spi3` enabled as spidev in `rk3566-microduck.dtsi`;
  ST ULD runs in userspace

## Sources

`vl53l8cx_api.c` / `vl53l8cx_buffers.h` etc. are ST's official ULD
(from stm32duino/VL53L8CX, identical to STSW-IMG040);
`platform_linux.c` is the spidev platform layer:

- SPI mode 3, MSB first, write address bit15=1, read address bit15=0
- Transfers split into 2048-byte chunks (spidev default bufsiz 4096)

## Build and Run

Preinstalled to `/opt/microduck-test/tof/` with the test-tools repo
during FULL image build and compiled in the chroot (`install.sh`);
run directly on the board:

```sh
sudo /opt/microduck-test/tof/vl53l8cx-test [spidev] [frames] [resolution 4|8] [fps Hz]
# defaults: /dev/spidev3.0, 20 frames, 8x8 (8x8 defaults to 15Hz, 4x4 to 60Hz)
sudo /opt/microduck-test/tof/vl53l8cx-test /dev/spidev3.0 10 4
```

Prints the per-frame N×N `distance_mm` matrix (only the last frame when
more than 20 frames), then the measured fps and a data check (count of
valid zones; status 5/6/9 count as valid).
Rebuild manually with `make -C /opt/microduck-test/tof`.

Note: `sudo systemctl stop tofd` must run first — two processes hitting
/dev/spidev3.0 at once interfere with each other and can wedge the
sensor (power cycle is the only recovery).
