#!/bin/bash

resolutions=("640x360" "640x480" "800x600" "1024x768" "1280x720" "1920x1080" "2560x1440" "3840x2160")

if [ "$#" -gt 0 ];then
    resolutions=($@)
fi

outhtml=/userdata/gstreamer_stress_test.html

echo "<table border=\"1\">
  <tr>
    <th rowspan=\"2\">resolutions</th>
    <th colspan=\"3\">encode</th>
    <th colspan=\"3\">decode</th>
  </tr>
  <tr>
    <th>H264</th>
    <th>H265</th>
    <th>JPEG</th>
    <th>H264</th>
    <th>H265</th>
    <th>JPEG</th>
  </tr>
" > $outhtml
for res in "${resolutions[@]}"; do
    IFS='x' read -r width height <<< "$res"
    echo "Resolution: $res, Width: $width, Height: $height"
    if [ $width -lt 800 ]; then
        # 60s
        buffers=1800
    elif [ $width -lt 1280 ]; then
        # 30s
        buffers=900
    else
        # 10s
        buffers=300
    fi
    blocksize=$((width*height*3/2))
    testfiledir=/userdata/gstvideotest
    rm $testfiledir -rf
    mkdir -p $testfiledir
    logfiledir=/tmp/gstvideotest
    mkdir -p $logfiledir
    rawfile=${testfiledir}/videotestsrc_${width}x${height}_nv12_${buffers}frames.yuv
    h264file=${testfiledir}/videotestsrc_${width}x${height}_nv12.h264
    h265file=${testfiledir}/videotestsrc_${width}x${height}_nv12.h265
    jpegfile=${testfiledir}/videotestsrc_${width}x${height}_nv12.jpeg
    h264enclog=${logfiledir}/h264enc_${width}x${height}.log
    h264declog=${logfiledir}/h264dec_${width}x${height}.log
    h265enclog=${logfiledir}/h265enc_${width}x${height}.log
    h265declog=${logfiledir}/h265dec_${width}x${height}.log
    jpegenclog=${logfiledir}/jpegenc_${width}x${height}.log
    jpegdeclog=${logfiledir}/jpegdec_${width}x${height}.log

    # generate raw file for testing
    echo "Generating raw file: $rawfile ..."
    gst-launch-1.0 videotestsrc num-buffers=$buffers ! video/x-raw,framerate=30/1,width=${width},height=${height},format=NV12 ! filesink location=$rawfile > /dev/null 2>&1

    if [ ! -f $rawfile ]; then
        echo "Failed to generate raw file: $rawfile"
        exit 1
    fi

    # generate h264 file for testing
    echo "Generating h264file: $h264file ..."
    gst-launch-1.0 filesrc location=$rawfile blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mpph264enc ! h264parse ! filesink location=$h264file > $h264enclog 2>&1

    if [ ! -f $h264file ]; then
        echo "Failed to generate h264 file: $h264file"
        cat $h264enclog
        exit 1
    fi

    # h264 encode test
    echo "H264 Testing ..."
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$rawfile blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mpph264enc ! fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $h264enclog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$h264enclog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$h264enclog")
    h264enc_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    h264enc_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo "======================"
    echo -e "${width}x${height} H264 encode\nmax fps: $h264enc_max_fps\nmin fps: $h264enc_min_fps"

    # h264 decode test
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$h264file ! h264parse ! mppvideodec ! \
        fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $h264declog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$h264declog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$h264declog")
    h264dec_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    h264dec_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo -e "${width}x${height} H264 decode\nmax fps: $h264dec_max_fps\nmin fps: $h264dec_min_fps"
    echo "======================"

    # generate h265 file for testing
    echo "Generating h265file: $h265file ..."
    gst-launch-1.0 filesrc location=$rawfile  blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mpph265enc ! h265parse ! filesink location=$h265file > $h265enclog 2>&1

    if [ ! -f $h265file ]; then
        echo "Failed to generate h265 file: $h265file"
        cat $h265enclog
        exit 1
    fi

    # h265 encode test
    echo "H265 Testing ..."
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$rawfile blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mpph265enc ! fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $h265enclog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$h265enclog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$h265enclog")
    h265enc_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    h265enc_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo -e "${width}x${height} H265 encode\nmax fps: $h265enc_max_fps\nmin fps: $h265enc_min_fps"

    # h265 decode test
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$h265file ! h265parse ! mppvideodec ! fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $h265declog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$h265declog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$h265declog")
    h265dec_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    h265dec_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo -e "${width}x${height} H265 decode\nmax fps: $h265dec_max_fps\nmin fps: $h265dec_min_fps"
    echo "======================"

    # generate jpeg file for testing
    echo "Generating jpegfile: $jpegfile ..."
    gst-launch-1.0 filesrc location=$rawfile  blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mppjpegenc ! jpegparse ! jifmux ! filesink location=$jpegfile > $jpegenclog 2>&1

    if [ ! -f $jpegfile ]; then
        echo "Failed to generate jpeg file: $jpegfile"
        cat $jpegenclog
        exit 1
    fi

    # jpeg encode test
    echo "JPEG Testing ..."
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$rawfile blocksize=$blocksize ! video/x-raw,width=$width,height=$height,format=NV12,framerate=30/1 ! mppjpegenc ! fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $jpegenclog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$jpegenclog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$jpegenclog")
    jpegenc_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    jpegenc_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo -e "${width}x${height} JPEG encode\nmax fps: $jpegenc_max_fps\nmin fps: $jpegenc_min_fps"

    # jpeg decode test
    GST_DEBUG=*fps*:7 gst-launch-1.0 filesrc location=$jpegfile ! jpegparse ! mppjpegdec ! fpsdisplaysink video-sink=fakesink text-overlay=false signal-fps-measurements=true sync=false > $jpegdeclog 2>&1
    last_max_fps_line=$(awk '/max-fps/ { line=$0 } END { print line }' "$jpegdeclog")
    last_min_fps_line=$(awk '/min-fps/ { line=$0 } END { print line }' "$jpegdeclog")
    jpegdec_max_fps=$(echo "$last_max_fps_line" | awk '{ print $NF }')
    jpegdec_min_fps=$(echo "$last_min_fps_line" | awk '{ print $NF }')
    echo -e "${width}x${height} JPEG decode\nmax fps: $jpegdec_max_fps\nmin fps: $jpegdec_min_fps"
    echo "======================"

    echo "<tr>
    <th>${width}x${height}</th>
    <th>max ${h264enc_max_fps}<br>min ${h264enc_min_fps}</th>
    <th>max ${h265enc_max_fps}<br>min ${h265enc_min_fps}</th>
    <th>max ${jpegenc_max_fps}<br>min ${jpegenc_min_fps}</th>
    <th>max ${h264dec_max_fps}<br>min ${h264dec_min_fps}</th>
    <th>max ${h265dec_max_fps}<br>min ${h265dec_min_fps}</th>
    <th>max ${jpegdec_max_fps}<br>min ${jpegdec_min_fps}</th>
  </tr>" >> $outhtml
done

echo "</table>" >> $outhtml
