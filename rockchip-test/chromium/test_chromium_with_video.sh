#!/bin/bash -e

PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

export mpp_syslog_perror=1

# Set debug parameters
KERNEL_VERSION=$(cat /proc/version)
if [[ $KERNEL_VERSION =~ "4.4" ]]; then
    echo 0x100 > /sys/module/rk_vcodec/parameters/debug
else
    echo 0x100 > /sys/module/rk_vcodec/parameters/mpp_dev_debug
fi

# Set performance mode
echo performance | tee $(find /sys/ -name *governor) /dev/null || true

# Improved chip detection logic
detect_chip() {
    local compatible
    compatible=$(cat /proc/device-tree/compatible 2>/dev/null || echo "")

    if [[ "$compatible" =~ "rk3328" ]]; then
        echo "rk3328"
    else
        echo "rockchip"
    fi
}

CHIPNAME=$(detect_chip)
echo "Detected chip: $CHIPNAME"

# Create device node and symbolic link
touch /dev/video-dec0
if [[ ! -e /usr/lib/libMali.so.1 ]]; then
    ln -sf /usr/lib/libmali.so /usr/lib/libMali.so.1
fi

# Chromium launch arguments
CHROMIUM_ARGS=(
    "--no-sandbox"
    "--gpu-sandbox-start-early"
    "--ozone-platform=wayland"
    "--ignore-gpu-blacklist"
    "--enable-wayland-ime"
)

# Adjust parameters based on chip type
if [[ "$CHIPNAME" == "rk3328" ]]; then
    CHROMIUM_ARGS+=("--in-process-gpu")
    echo "Applying RK3328 specific optimizations"
fi

# Launch Chromium
if [[ -e "/usr/bin/chromium" ]]; then
    chromium "${CHROMIUM_ARGS[@]}" "file:///oem/SampleVideo_1280x720_5mb.mp4"
else
    echo "Error: Chromium not found at /usr/bin/chromium"
    echo "Please ensure the config/rockchip_xxxx_defconfig includes 'chromium.config'"
    exit 1
fi

echo "The governor is set to performance mode. A restart may be required for full effect."
