#!/bin/bash

URI=/oem/SampleVideo_1280x720_5mb.mp4

export mpp_syslog_perror=1

# Hardware detection logic
if ls /dev/mali* 1>/dev/null 2>&1; then
    SINK="waylandsink"
else
    SINK="kmssink"
fi

if [ "$1" != "" ]
then
    URI=$1
    if [ "${URI:0:1}" != "/" ]
    then
        URI=$(readlink -f $URI)
    fi
fi

if [ "${URI:0:1}" == "/" ]
then
    URI=file://$URI
fi

gst-launch-1.0 uridecodebin uri=$URI ! "$SINK" fullscreen=true
