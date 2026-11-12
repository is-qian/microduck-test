#!/bin/sh
# Enter MP verify mode. Connect the RF port to the instrument BEFORE running:
# dstart does the RF self-calibration, needed only once after each power-up.
set -e

ioctl_app dtop "08 00 00 00 2e 00 00 01"
ioctl_app wifi "dstart 15 1 0"
ioctl_app wifi "mp_mode_en 0"
