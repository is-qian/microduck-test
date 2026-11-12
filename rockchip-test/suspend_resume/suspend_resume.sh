#!/bin/bash

RESULT_DIR=/userdata/rockchip-test/
RESULT_LOG=${RESULT_DIR}/suspend_resume.txt
SUSPEND_TIME=60
WAKE_DELAY=5
MAX_CYCLES=10000

# Get device compatibility (handle null delimiter)
COMPATIBLE=$(tr '\0' ' ' < /proc/device-tree/compatible)

# Debug options
# To enable the debug configuration, the CONFIG_RK_CONSOLE_THREAD in the kernel configuration needs to be disabled.
# echo N > /sys/module/printk/parameters/console_suspend
# echo 1 > /sys/power/pm_print_times

# Verify RTC existence
if [ ! -e "/sys/class/rtc/rtc0/wakealarm" ]; then
    echo "RTC not detected, please check RTC configuration"
    exit 1
fi

# Platform-specific configuration
if [[ "$COMPATIBLE" =~ "rv1126b" ]]; then
    echo "RV1126B platform detected, skipping DMC governor and wake interval setup"
    # For non-RV1126B platforms:
    # 2. Configure hardware wake interval (0x8000 = 1 second wake interval)
    io -4 0x20834310 0x8000
    SUSPEND_TIME=1
else
    continue
fi

# Create result directory
mkdir -p ${RESULT_DIR}

auto_suspend_resume_rtc() {
    local cnt=0

    # Sync system time to RTC
    hwclock --systohc
    echo "$(date): Automated suspend/resume test initiated" > ${RESULT_LOG}

    while [ $cnt -lt $MAX_CYCLES ]; do
        echo "Completed $cnt suspend/resume cycles"

        # Set RTC wakeup timer
        echo 0 > /sys/class/rtc/rtc0/wakealarm
        echo "+${SUSPEND_TIME}" > /sys/class/rtc/rtc0/wakealarm

        # Initiate system suspend
        pm-suspend

        # Post-wakeup handling
        echo "System resumed, next cycle in ${WAKE_DELAY} seconds"
        sleep ${WAKE_DELAY}

        # Log status
        echo "$(date): Cycle: $cnt - Suspend: ${SUSPEND_TIME}s Wake delay: ${WAKE_DELAY}s" >> ${RESULT_LOG}

        cnt=$((cnt + 1))
    done
}

# Start test sequence
auto_suspend_resume_rtc
