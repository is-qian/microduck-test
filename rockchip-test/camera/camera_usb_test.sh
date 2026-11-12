#!/bin/bash
#export GST_DEBUG=*:5
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/usr/lib/gstreamer-1.0

# Check GPU device nodes
shopt -s nullglob
mali_devices=(/dev/mali*)
shopt -u nullglob

if [ ${#mali_devices[@]} -gt 0 ]; then
    sink="waylandsink"
else
    sink="kmssink"
fi
echo "Selected sink: $sink"

# Get USB camera device list
v4l2-ctl --list-devices > /tmp/.v4l2_list
USB_VIDEO=($(awk '/usb/{getline a;print a}' /tmp/.v4l2_list))
echo "Found ${#USB_VIDEO[@]} USB Cameras"
rm /tmp/.v4l2_list

# Iterate through all USB cameras
for device in "${USB_VIDEO[@]}"; do
    echo "Starting preview for USB Camera at $device with $sink"
    gst-launch-1.0 -e v4l2src device="$device" ! \
        image/jpeg ! jpegparse ! mppjpegdec ! \
        $sink sync=false
done
