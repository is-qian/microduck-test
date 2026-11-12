#!/bin/sh
# Fixed-frequency RX (PER test). MP mode must be active.
# Usage: rx.sh <channel> <bw> [ant]     -- configure RX path, clear counters
# Then let the instrument transmit, and query PER with:
#   ioctl_app wifi "rxratecnt <mode> <rate>"
#   mode: 0=b/g/a  2=n  4=ac  5=ax   rate: same as tx.sh
set -e

[ $# -ge 2 ] || { sed -n '2,8p' "$0"; exit 1; }
CH=$1; BW=$2; ANT=${3:-1}

case $BW in
  20) SBW=0; OFF=0 ;;
  40) SBW=1; OFF=2 ;;
  80) SBW=2; OFF=6 ;;
  *) echo "invalid bw: $BW"; exit 1 ;;
esac
CH=$((CH - OFF))

ioctl_app wifi "rxratecnt 0 r"
ioctl_app wifi "set_ch $CH $SBW 0 0"
ioctl_app wifi "rx_ant_cfg $ANT"
echo "RX ready: ch=$1 bw=${BW}M ant=$ANT (counters cleared)"
