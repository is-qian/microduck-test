# hwtest

Menu-driven hardware test tool for the MicroDuck D4 board. Test items
mirror `test/hardware-test.md`: pick a module from the main menu, pick
an action, fill in parameters when asked, then judge the result
(pass/fail/skip). `b` goes back one level, `m` returns to the main
menu, `q` quits; a summary is written to `/tmp/hwtest-report.txt`.

The last main-menu entry "Auto full check" runs an unattended sequence (WiFi
scan, BT scan, IMU, camera snapshot, ToF, speaker/mic loopback) while
all face LEDs blink at 500ms and the fan sweeps low->high. It needs an
audio file (default `/opt/microduck-test/hwtest/bbno.wav`); drop a
`bbno.wav` into this directory to ship it with the image.

Usage on the board:

```sh
sudo /opt/microduck-test/hwtest/hwtest          # interactive menus
sudo /opt/microduck-test/hwtest/hwtest --list   # print the menu tree
```

The image ships the binary built by the dist CI (see ../README.md). To
build locally (requires the `aarch64-unknown-linux-musl` rust target;
statically linked via rust-lld, so the dev host glibc version does not
matter):

```sh
./build.sh
```
