#!/bin/sh
# Fixed-frequency TX. MP mode must be active (see load_mp_ko.sh/enter_mp_mode.sh).
# Usage: tx.sh <channel> <mode> <rate> <bw> <power_dbm> [ant]
#   mode: 0=b/g/a  2=n  4=ac  5=ax
#   rate: b/g/a: 1 2 5.5 11 6 9 12 18 24 36 48 54 | n: 0-7 | ac: 0-9 | ax: 0-11
#   bw:   20/40/80 (MHz)
#   ant:  1 or 2 (default 1)
# Stop: tx.sh cancel
set -e

[ "$1" = "cancel" ] && exec ioctl_app wifi "cancel"

[ $# -ge 5 ] || { sed -n '2,10p' "$0"; exit 1; }
CH=$1; MODE=$2; RATE=$3; BW=$4; POWER=$5; ANT=${6:-1}

# set_ch wants the primary-20 channel: center-2 for 40M, center-6 for 80M
case $BW in
  20) SBW=0; OFF=0; PKTLEN=1000 ;;
  40) SBW=1; OFF=2; PKTLEN=1000 ;;
  80) SBW=2; OFF=6; PKTLEN=1500 ;;
  *) echo "invalid bw: $BW"; exit 1 ;;
esac
CH=$((CH - OFF))

# 11b needs trailing "1 1" in tx_mpdu; everything else uses "16 2"
TAIL="16 2"
if [ "$MODE" = "0" ]; then
  case $RATE in 1|2|5.5|11) TAIL="1 1" ;; esac
fi

ioctl_app wifi "tx_ant_cfg 0 $ANT 0"
ioctl_app wifi "set_ch $CH $SBW 0 0"
ioctl_app wifi "rx_ant_cfg $ANT"
ioctl_app wifi "force_cca 0 1"
ioctl_app wifi "force_cca 1 1"
ioctl_app wifi "ampdu_prot 0"
ioctl_app wifi "set_rate $MODE $RATE $BW 0 0 0 1"
ioctl_app wifi "set_power 0 $POWER"
ioctl_app wifi "tx_mpdu 0 $PKTLEN 0x01 3 2 0 0 50 $TAIL"
echo "TX started: ch=$1 bw=${BW}M mode=$MODE rate=$RATE power=${POWER}dBm ant=$ANT"
