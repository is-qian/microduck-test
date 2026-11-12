#!/bin/bash

URI=/oem/SampleVideo_1280x720_5mb.mp4

export mpp_syslog_perror=1

if [ "$1" != "" ]; then
    URI=$1
    if [ "${URI:0:1}" != "/" ]; then
        URI=$(readlink -f $URI)
    fi
fi

if [ "${URI:0:1}" == "/" ]; then
    URI=file://$URI
fi

# Hardware detection logic
if ls /dev/mali* >/dev/null 2>&1; then
    VIDEO_SINK_NAME="waylandsink"
    VIDEO_SINK_ARGS="fullscreen=true"
else
    VIDEO_SINK_NAME="kmssink"
    VIDEO_SINK_ARGS=""
fi

GST_DEBUG=fps*:7 gst-launch-1.0 uridecodebin uri=$URI ! fpsdisplaysink \
    video-sink="$VIDEO_SINK_NAME" \
    $VIDEO_SINK_ARGS \
    text-overlay=false \
    signal-fps-measurements=true \
    sync=false
