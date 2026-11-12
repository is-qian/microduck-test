#!/bin/sh
# Build both NFC tools. CC is overridable for cross builds
# (CI uses aarch64-linux-gnu-gcc; a native chroot/dev build uses gcc).
set -e
cd "$(dirname "$0")"
CC=${CC:-gcc}
"$CC" -static -O2 -o nfc-id-test nfc-id-test.c
"$CC" -static -O2 -Wall -Wno-unused-parameter -Wno-sign-compare \
    -DST25R200 \
    -I. -Irfal/include -Irfal/source -Irfal/source/st25r200 \
    -o nfc-test \
    nfc-test.c rfal_platform_linux.c \
    rfal/source/rfal_analogConfig.c \
    rfal/source/rfal_cd.c \
    rfal/source/rfal_crc.c \
    rfal/source/rfal_dpo.c \
    rfal/source/rfal_iso15693_2.c \
    rfal/source/rfal_isoDep.c \
    rfal/source/rfal_nfc.c \
    rfal/source/rfal_nfca.c \
    rfal/source/rfal_nfcb.c \
    rfal/source/rfal_nfcDep.c \
    rfal/source/rfal_nfcf.c \
    rfal/source/rfal_nfcv.c \
    rfal/source/rfal_st25tb.c \
    rfal/source/rfal_st25xv.c \
    rfal/source/rfal_t1t.c \
    rfal/source/rfal_t2t.c \
    rfal/source/rfal_t4t.c \
    rfal/source/st25r200/rfal_rfst25r200.c \
    rfal/source/st25r200/st25r200.c \
    rfal/source/st25r200/st25r200_com.c \
    rfal/source/st25r200/st25r200_irq.c -lm
