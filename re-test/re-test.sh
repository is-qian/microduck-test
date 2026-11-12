#!/bin/bash
# MicroDuck full-function RE (run-in) test: BT gamepad, WiFi scan, camera,
# speaker, mic, NFC poll, ToF ranging, face IMU, LEDs, fan, dynamixel
# servos (zero + slow +/-25deg swing) all running concurrently.
# Must run as root on the board. See README.md.
set -u

# precedence: env > re-test.conf (next to script or in $PWD) > built-in defaults
load_conf() {
    local conf=$1 line key
    [ -f "$conf" ] || return 0
    while IFS= read -r line; do
        case "$line" in ''|'#'*) continue;; esac
        key=${line%%=*}
        [ -z "${!key+x}" ] && eval "$line"
    done < "$conf"
}
SELF=$(readlink -f "$0" 2>/dev/null || echo "$0")
load_conf "$(dirname "$SELF")/re-test.conf"
load_conf "$PWD/re-test.conf"

export DXL=${DXL:-/opt/microduck-test/dxl/dxl}
export DXL_PORT=${DXL_PORT:-/dev/ttyS3}  # pass DXL_PORT=/dev/ttyS2 on the Yocto image
TONE=${TONE:-/opt/microduck-test/tone.wav}  # pre-place it; boards without ffmpeg cannot render one
export TOF=${TOF:-/opt/microduck-test/tof/vl53l8cx-test}
export NFC=${NFC:-/opt/microduck-test/nfc/nfc-test}
DURATION=${DURATION:-0}        # seconds, 0 = run until Ctrl-C
TEMP_MAX=${TEMP_MAX:-85000}    # millidegree, overheat protection
export BT_MAC=${BT_MAC:-}      # paired gamepad MAC; bt module skipped if empty
export SERVO_IDS=${SERVO_IDS:-auto} # "1 2 3", or auto = dxl scan minus GD32 (200)
SERVO_BAUD=${SERVO_BAUD:-1000000}   # target bus baud: 57600 or 1000000; devices are reconfigured first if needed
case "$SERVO_BAUD" in 57600|1000000) ;; *) echo "SERVO_BAUD must be 57600 or 1000000"; exit 1;; esac
SKIP=${SKIP:-}                 # space separated modules: bt wifi cam spk mic nfc tof imu led fan servo
export MIC_REC=${MIC_REC:-5}   # mic: seconds recording per cycle
export MIC_PAUSE=${MIC_PAUSE:-5} # mic: seconds paused per cycle
FAN_PWM=${FAN_PWM:-255}        # 0-255

export SERVO_CENTER=${SERVO_CENTER:-2048}
export SERVO_AMP=${SERVO_AMP:-284}  # 25 deg = 4096 * 25 / 360
export SERVO_VEL=${SERVO_VEL:-10}       # profile velocity, 0.229 rpm/unit -> ~1.8 s per 25 deg swing
export SERVO_PERIOD=${SERVO_PERIOD:-3}  # seconds per half-swing

export LOG=/tmp/re-test/$(date +%Y%m%d_%H%M%S)
mkdir -p "$LOG/state"
PIDS=()
NAMES=()
TOFD_STOPPED=0

skip() { case " $SKIP " in *" $1 "*) return 0;; *) return 1;; esac }
log() { echo "[$(date +%T)] $*" | tee -a "$LOG/main.log"; }

# every module runs under setsid so cleanup can kill the whole process group
spawn() { local name=$1; shift; setsid "$@" & PIDS+=($!); NAMES+=("$name"); }

# restart wrapper for simple long-running commands
runner() { local logf=$1; shift
    while true; do stdbuf -oL -eL "$@" >>"$logf" 2>&1
        echo "--- exited rc=$?, restart ---" >>"$logf"; sleep 1; done; }
export -f runner
run() { local name=$1; shift; spawn "$name" bash -c "runner $LOG/$name.log $*"; }

# ---- bluetooth gamepad ----
bt_loop() {
    n=0
    while true; do
        info=$(bluetoothctl info "$BT_MAC" 2>&1)
        if echo "$info" | grep -q "Connected: yes"; then
            echo "$(date +%T) connected"
        else
            # unknown to bluetoothd until discovered once
            if echo "$info" | grep -q "not available"; then
                echo "$(date +%T) unknown device, scanning"
                bluetoothctl --timeout 15 scan on >/dev/null 2>&1
            fi
            echo "$(date +%T) pair/trust/connect"
            bluetoothctl pair "$BT_MAC" 2>&1
            bluetoothctl trust "$BT_MAC" 2>&1
            bluetoothctl connect "$BT_MAC" 2>&1
        fi
        sleep 5
        # periodic discovery scan: BT/WiFi coexistence stress
        n=$((n + 1)); [ $((n % 12)) -eq 0 ] && bluetoothctl --timeout 5 scan on
    done >>"$LOG/bt.log" 2>&1
}
export -f bt_loop
bt_start() {
    [ -n "$BT_MAC" ] || { log "bt: BT_MAC not set, skipped"; return 1; }
    if command -v evtest >/dev/null; then
        for d in /sys/class/input/event*/device/name; do
            if grep -qiE 'gamepad|controller|joystick|xbox|dualsense|8bitdo|pro controller' "$d" 2>/dev/null; then
                spawn bt-events evtest "/dev/input/$(basename "$(dirname "$(dirname "$d")")")" \
                    >>"$LOG/bt-events.log" 2>&1
            fi
        done
    fi
    spawn bt bash -c bt_loop
}

# ---- wifi continuous scan ----
wifi_loop() {
    while true; do
        iw dev wlan0 scan trigger >/dev/null 2>&1; sleep 3
        n=$(iw dev wlan0 scan dump 2>/dev/null | grep -c '^BSS ')
        l=$(iw dev wlan0 link 2>/dev/null | grep -c 'Connected')
        echo "$(date +%T) bss=$n link=$l"
        echo "${n}AP$([ "$l" = 1 ] && echo +link)" >"$LOG/state/wifi"
        sleep 7
    done >>"$LOG/wifi.log" 2>&1
}
export -f wifi_loop
wifi_start() { spawn wifi bash -c wifi_loop; }

# ---- camera ----
cam_start() {
    media-ctl -d /dev/media0 -p 2>/dev/null | grep -qE 'gc2093|gc2613' \
        || { log "cam: no sensor probed, skipped"; return 1; }
    run cam v4l2-ctl -d /dev/video0 --stream-mmap=4 --stream-count=900 --stream-to=/dev/null
}

# ---- speaker ----
spk_start() {
    aplay -l 2>/dev/null | grep -q 'card 0' || { log "spk: no sound card, skipped"; return 1; }
    amixer -c 0 sset 'Playback Path' SPK_HP >/dev/null 2>&1
    amixer -c 0 sset 'DAC' 200 >/dev/null 2>&1
    [ -s "$TONE" ] || ffmpeg -v error -y -f lavfi -i "sine=frequency=880:duration=30" -ar 48000 -ac 2 "$TONE" || return 1
    run spk aplay -q -D plughw:0,0 "$TONE"
}

# ---- mic: record 5s, pause 5s, repeat ----
mic_loop() {
    while true; do
        arecord -q -D hw:facemic,0 -f S32_LE -r 16000 -c 2 -d "$MIC_REC" /dev/null || sleep 2
        sleep "$MIC_PAUSE"
    done >>"$LOG/mic.log" 2>&1
}
export -f mic_loop
mic_start() {
    arecord -l 2>/dev/null | grep -q facemic || { log "mic: facemic not found, skipped"; return 1; }
    spawn mic bash -c mic_loop
}

# ---- nfc poll ----
nfc_loop() {
    while true; do
        out=$("$NFC" poll 5000 2>&1)
        echo "$out" >>"$LOG/nfc.log"
        uid=$(echo "$out" | grep -o 'nfcid=[0-9A-Fa-f]*' | tail -1)
        echo "${uid:-no-tag}" >"$LOG/state/nfc"
        sleep 1
    done
}
export -f nfc_loop
nfc_start() { spawn nfc bash -c nfc_loop; }

# ---- face board IMU (LSM6DSV16X, i2c3 0x6a) ----
imu_init() { i2cset -y 3 0x6a 0x12 0x01; sleep 0.1; i2cset -y 3 0x6a 0x12 0x44
             i2cset -y 3 0x6a 0x10 0x06; i2cset -y 3 0x6a 0x11 0x06; }
imu_stop() { i2cset -y 3 0x6a 0x10 0x00 2>/dev/null; i2cset -y 3 0x6a 0x11 0x00 2>/dev/null; }
imu_loop() {
    while true; do echo "$(date +%T) $(i2cget -y 3 0x6a 0x22 i 12 2>&1 | tr '\n' ' ')"; sleep 1
    done >>"$LOG/imu.log" 2>&1
}
export -f imu_loop
imu_start() {
    [ "$(i2cget -y 3 0x6a 0x0f 2>/dev/null)" = "0x70" ] || { log "imu: WHO_AM_I fail, skipped"; return 1; }
    imu_init
    spawn imu bash -c imu_loop
}

# ---- LEDs all on ----
led_start() {
    ls -d /sys/class/leds/face:* >/dev/null 2>&1 || { log "led: no face leds, skipped"; return 1; }
    for l in /sys/class/leds/face:*; do
        echo none >"$l/trigger" 2>/dev/null
        echo 255 >"$l/brightness" 2>/dev/null
    done
}
led_stop() {
    for l in /sys/class/leds/face:*; do [ -e "$l" ] || continue; echo 0 >"$l/brightness" 2>/dev/null; done
}

# ---- fan full speed + tach ----
FANHWMON=$(grep -l pwmfan /sys/class/hwmon/hwmon*/name 2>/dev/null | head -1 | xargs -r dirname)
export FANHWMON
fan_loop() {
    while true; do
        p=$(timeout 2 gpiomon -r gpiochip3 8 2>/dev/null | wc -l)
        echo "$(date +%T) tach_hz=$((p / 2)) pwm=$(cat "$FANHWMON/pwm1" 2>/dev/null)"
        echo "$((p / 2))Hz" >"$LOG/state/fan"
        sleep 28
    done >>"$LOG/fan.log" 2>&1
}
export -f fan_loop
fan_start() {
    [ -n "$FANHWMON" ] || { log "fan: pwmfan hwmon not found, skipped"; return 1; }
    echo 1 >"$FANHWMON/pwm1_enable"
    echo "$FAN_PWM" >"$FANHWMON/pwm1"
    spawn fan bash -c fan_loop
}
fan_stop() { [ -n "$FANHWMON" ] && echo 0 >"$FANHWMON/pwm1" 2>/dev/null; }

# ---- tof ranging ----
tof_loop() {
    while true; do
        out=$(stdbuf -oL -eL "$TOF" /dev/spidev3.0 300 8 2>&1)  # 300 frames @15Hz = 20s
        echo "$out" >>"$LOG/tof.log"
        fps=$(echo "$out" | grep -o '[0-9.]* fps' | tail -1)
        echo "${fps:-fail}" >"$LOG/state/tof"
        sleep 1
    done
}
export -f tof_loop

# ---- dynamixel servos + GD32 sensors (single loop owns ttyS3) ----
BUS_BAUD=1000000                  # baud the devices currently answer at
SERVO_BAUD_SWITCHED=0
dxlb() { "$DXL" -b "$BUS_BAUD" "$@"; }
baud_code() { [ "$1" = 57600 ] && echo 01 || echo 03; }
other_baud() { [ "$1" = 57600 ] && echo 1000000 || echo 57600; }
u32le() { local v=$1; printf '%02x%02x%02x%02x' $((v & 255)) $(((v >> 8) & 255)) $(((v >> 16) & 255)) $(((v >> 24) & 255)); }
le2dec() { local h=${1#*data=} b="" i; [ -n "$h" ] || { echo -; return; }
    for ((i = ${#h} - 2; i >= 0; i -= 2)); do b="$b${h:i:2}"; done
    echo $((16#$b)); }
export -f u32le le2dec dxlb
export BUS_BAUD DXL
servo_goal() { for id in $SERVO_IDS; do dxlb write "$id" 116 "$(u32le "$1")" >>"$LOG/servo.log" 2>&1; done; }
export -f servo_goal
servo_loop() {
    hi=$((SERVO_CENTER + SERVO_AMP)); lo=$((SERVO_CENTER - SERVO_AMP)); goal=$hi; tick=0
    while true; do
        servo_goal $goal
        [ $goal -eq $hi ] && goal=$lo || goal=$hi
        sleep "$SERVO_PERIOD"
        tick=$((tick + 1))
        if [ $((tick % 10)) -eq 0 ]; then
            st=""
            { echo "== $(date +%T) status =="
              for id in $SERVO_IDS; do
                  pos=$(dxlb read "$id" 132 4); tmp=$(dxlb read "$id" 146 1)
                  volt=$(dxlb read "$id" 144 2); hwe=$(dxlb read "$id" 70 1)
                  echo "id=$id pos=$pos temp=$tmp volt=$volt hwerr=$hwe"
                  st="$st $id:$(le2dec "$pos")@$(le2dec "$tmp")C"
              done
              sens=$(dxlb sensors 2>&1); echo "$sens"
              bat=$(echo "$sens" | awk '/voltage:/{print $2}')
              echo "$st bat:${bat:-?}V" >"$LOG/state/servo"
            } >>"$LOG/servo.log" 2>&1
        fi
    done
}
export -f servo_loop
servo_init() {
    for id in $SERVO_IDS; do
        { dxlb write "$id" 64 01; dxlb write "$id" 112 "$(u32le "$SERVO_VEL")"
          dxlb write "$id" 116 "$(u32le "$SERVO_CENTER")"; } >>"$LOG/servo.log" 2>&1
    done
    sleep 3
    for id in $SERVO_IDS; do
        pos=$(dxlb read "$id" 132 4 2>/dev/null | grep -o 'data=.*' | tr -d 'data= ')
        echo "$(date +%T) id=$id zero_check raw_pos=${pos:-no-answer}" >>"$LOG/servo.log"
    done
}
servo_stop() {
    servo_goal $SERVO_CENTER; sleep 2
    for id in $SERVO_IDS; do dxlb write "$id" 64 00 >/dev/null 2>&1; done
    if [ "$SERVO_BAUD_SWITCHED" = 1 ]; then   # robot software expects the bus @1M
        for id in $SERVO_IDS 200; do dxlb write "$id" 8 03 >/dev/null 2>&1; done
        log "servo: bus baud restored to 1000000"
    fi
}
servo_alive() { # ids... -> 0 if every id pings
    local id
    for id in "$@"; do dxlb ping "$id" >/dev/null 2>&1 || return 1; done
}
servo_set_baud() { # switch all bus devices (servos + GD32 200) to $SERVO_BAUD
    local id
    log "servo: reconfiguring bus @$BUS_BAUD -> @$SERVO_BAUD"
    for id in $SERVO_IDS 200; do
        dxlb write "$id" 64 00 >/dev/null 2>&1   # XL330 baud is EEPROM: torque off first
        dxlb write "$id" 8 "$(baud_code "$SERVO_BAUD")" >/dev/null 2>&1
    done
    sleep 1
    BUS_BAUD=$SERVO_BAUD; export BUS_BAUD
    SERVO_BAUD_SWITCHED=1
    for id in $SERVO_IDS 200; do
        if servo_alive "$id"; then echo "$(date +%T) id=$id baud ok @$SERVO_BAUD" >>"$LOG/servo.log"
        else echo "$(date +%T) id=$id NO ANSWER @$SERVO_BAUD" >>"$LOG/servo.log"; fi
    done
}
servo_start() {
    BUS_BAUD=$SERVO_BAUD
    if [ "$SERVO_IDS" = auto ]; then
        log "servo: scanning bus @$SERVO_BAUD ..."
        SERVO_IDS=$(dxlb scan "$SERVO_BAUD" 2>/dev/null | grep "^@$SERVO_BAUD" |
            grep -o '([0-9]\+,' | tr -d '(,' | grep -vx 200 | tr '\n' ' ')
        export SERVO_IDS
        log "servo: found IDs: ${SERVO_IDS:-none}"
    fi
    if [ -z "$SERVO_IDS" ] || ! servo_alive $SERVO_IDS 200; then
        # devices may sit at the other baud: switch them over
        BUS_BAUD=$(other_baud "$SERVO_BAUD")
        if [ -z "$SERVO_IDS" ]; then
            log "servo: scanning bus @$BUS_BAUD ..."
            SERVO_IDS=$(dxlb scan "$BUS_BAUD" 2>/dev/null | grep "^@$BUS_BAUD" |
                grep -o '([0-9]\+,' | tr -d '(,' | grep -vx 200 | tr '\n' ' ')
            export SERVO_IDS
            log "servo: found IDs: ${SERVO_IDS:-none}"
        fi
        [ -n "$SERVO_IDS" ] && servo_alive $SERVO_IDS 200 && servo_set_baud
    fi
    BUS_BAUD=$SERVO_BAUD; export BUS_BAUD
    [ -n "$SERVO_IDS" ] || { log "servo: no servos on bus, skipped"; return 1; }
    servo_alive $SERVO_IDS || { log "servo: not all IDs answer @$SERVO_BAUD, skipped"; return 1; }
    servo_init
    spawn servo bash -c servo_loop
}

cleanup() {
    log "stopping all modules, restoring system"
    for pid in "${PIDS[@]:-}"; do [ -n "$pid" ] && kill -- -"$pid" 2>/dev/null; done
    sleep 1
    for pid in "${PIDS[@]:-}"; do [ -n "$pid" ] && kill -9 -- -"$pid" 2>/dev/null; done
    [ -n "${SERVO_IDS:-}" ] && [ "$SERVO_IDS" != auto ] && servo_stop
    led_stop; fan_stop; imu_stop
    [ "$TOFD_STOPPED" = 1 ] && systemctl start tofd 2>/dev/null
    log "done, logs: $LOG"
    exit 0
}
trap cleanup SIGINT SIGTERM

[ "$(id -u)" -eq 0 ] || { echo "must run as root"; exit 1; }
command -v evtest >/dev/null || log "note: evtest not found, gamepad events not captured (connection still monitored)"

log "RE test start, logs: $LOG"

start_mod() { # name command...
    local name=$1; shift
    if skip "$name"; then log "module $name: skipped by user"; return; fi
    if "$@"; then log "module $name: on"; fi
}

start_mod led   led_start
start_mod fan   fan_start
start_mod imu   imu_start
start_mod spk   spk_start
start_mod mic   mic_start
start_mod cam   cam_start
start_mod nfc   nfc_start
if ! skip tof; then
    systemctl stop tofd 2>/dev/null && TOFD_STOPPED=1
    spawn tof bash -c tof_loop
    log "module tof: on"
fi
start_mod wifi  wifi_start
start_mod bt    bt_start
start_mod servo servo_start

for i in "${!NAMES[@]}"; do log "started: ${NAMES[$i]} pid=${PIDS[$i]}"; done

START=$(date +%s)
declare -A DEAD=()
tick=0
echo "time,temp_soc_c,cpu_khz,modules_alive" >"$LOG/summary.csv"
while true; do
    T=$(cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null || echo 0)
    F=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq 2>/dev/null || echo 0)
    alive=0
    for i in "${!PIDS[@]}"; do
        pid=${PIDS[$i]}
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            alive=$((alive + 1))
        elif [ -z "${DEAD[$i]:-}" ]; then
            DEAD[$i]=1; log "module ${NAMES[$i]} (pid $pid) died"
        fi
    done
    echo "$(date +%T),$((T / 1000)),$F,$alive/${#PIDS[@]}" >>"$LOG/summary.csv"
    tick=$((tick + 1))
    if [ $((tick % 6)) -eq 1 ]; then
        st=""
        for m in servo fan wifi nfc tof; do
            [ -f "$LOG/state/$m" ] && st="$st | $m:$(cat "$LOG/state/$m")"
        done
        log "STATUS $((T / 1000))C $((F / 1000))MHz alive=$alive/${#PIDS[@]}$st"
    fi
    [ "$T" -ge "$TEMP_MAX" ] && { log "overheat $((T / 1000))C, stop"; cleanup; }
    [ "$DURATION" -gt 0 ] && [ $(($(date +%s) - START)) -ge "$DURATION" ] && { log "duration reached"; cleanup; }
    sleep 10
done
