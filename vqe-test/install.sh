#!/bin/sh
# Dist build entry (see ../build-dist.sh): build and install the runtime
# payload into $1 — layout mirrors hwtest, self-contained under
# /opt/microduck-test/vqe-test/ on the board. Not shipped in the payload.
set -e
cd "$(dirname "$0")"
./build.sh
install -D -m 755 out/vqe_test "$1/vqe_test"
install -m 755 out/vqe_live "$1/vqe_live"
install -m 755 out/librkaudio_vqe.so out/librkaudio_common.so "$1/"
install -m 755 vqe-test vqe-live "$1/"
install -m 644 config_mic.json test-in.wav README.md "$1/"
