#!/bin/sh
# Cross-build both vqe binaries (host side, also used by CI).
# The closed-source libs are vendored under vendor/, so no SDK checkout is
# needed; set SDK=/path/to/sdk to build against one instead.
# gnu target (not musl like hwtest): the vendored librkaudio_vqe.so is
# glibc-built, so the binaries must link it dynamically.
set -e
cd "$(dirname "$0")"
CC=${CC:-aarch64-linux-gnu-gcc}
VENDOR=./vendor

[ -x "$(command -v $CC)" ] || { echo "need $CC (apt install gcc-aarch64-linux-gnu)"; exit 1; }

if [ -d "$VENDOR/lib64" ]; then
	LIBDIR=$VENDOR/lib64
else
	SDK=${SDK:-$HOME/work/rk/sdk-pull/rk3566_rk3568-linux6.1}
	ALGO=$SDK/external/common_algorithm/audio/rkaudio_algorithms
	[ -d "$ALGO" ] || { echo "neither ./vendor nor SDK found (override with SDK=...)"; exit 1; }
	LIBDIR=$ALGO/lib64
fi

cargo build --release --target aarch64-unknown-linux-gnu

mkdir -p out
cp target/aarch64-unknown-linux-gnu/release/vqe_test \
   target/aarch64-unknown-linux-gnu/release/vqe_live out/
cp "$LIBDIR"/librkaudio_vqe.so "$LIBDIR"/librkaudio_common.so out/
echo "libs from: $LIBDIR"
