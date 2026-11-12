#!/bin/bash
#export GST_DEBUG=*:5

# Hardware detection logic
if ls /dev/mali* 1>/dev/null 2>&1; then
    SINK="waylandsink"
else
    SINK="kmssink"
fi

# Unified camera device path
CAM_DEVICE="/dev/video-camera0"

# Enhanced device detection (with fallback paths)
declare -a CAM_PATHS=(
    "$CAM_DEVICE"
    "/dev/video0"
    "/dev/video1"
)

for cam in "${CAM_PATHS[@]}"; do
    if [ -e "$cam" ]; then
        CAM_DEVICE="$cam"
        break
    fi
done

if [ ! -e "$CAM_DEVICE" ]; then
    echo "Error: No available camera device found, tried:"
    printf '• %s\n' "${CAM_PATHS[@]}"
    v4l2-ctl --list-devices 2>/dev/null || echo "Please install v4l-utils to list devices"
    exit 1
fi

# Verify basic camera capabilities
if ! v4l2-ctl -d "$CAM_DEVICE" --all >/dev/null 2>&1; then
    echo "Camera device $CAM_DEVICE inaccessible"
    exit 2
fi

# Set library path
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/usr/lib/gstreamer-1.0

# Launch GStreamer pipeline
gst-launch-1.0 -v v4l2src device="$CAM_DEVICE" ! \
    video/x-raw,format=NV12,width=640,height=480,framerate=30/1 ! \
    "$SINK"
