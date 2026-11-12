#!/bin/sh
# Reload wq_wlan with MP (factory test) firmware. BT becomes unavailable.
set -e

pkill btattach 2>/dev/null || true
hciconfig hci0 down 2>/dev/null || true
lsmod | grep -q '^wq_wlan' && rmmod wq_wlan

KVER=$(uname -r)
KO=""
for p in "/lib/modules/$KVER/wq_wlan.ko" "/lib/modules/$KVER/updates/dkms/wq_wlan.ko"; do
  if [ -f "$p" ]; then KO=$p; break; fi
done
[ -n "$KO" ] || { echo "wq_wlan.ko not found under /lib/modules/$KVER"; exit 1; }

insmod "$KO" \
  fw_dtop_sdio=wq9201_fw_dtop_1_1_mp_sdio.bin \
  fw_bt=wq9201_fw_bt_1_1_mp.bin \
  fw_wifi_sdio=wq9201_fw_wifi_1_1_dtest.bin \
  oem_name=wq9201_oem_1_1_0366.bin \
  wifi_phy_name=wq9201_phy_1_1_4366_0102.bin \
  loadfw_only=1 fw_sys=5 force_reset=1

# mp_test appears only after the MP firmware is up; ioctl_app fails before that
i=0
while [ ! -e /proc/driver/wifi_mp/mp_test ]; do
  i=$((i + 1))
  [ $i -gt 30 ] && { echo "timeout waiting for /proc/driver/wifi_mp/mp_test"; exit 1; }
  sleep 1
done
echo "MP driver ready: $KO"
