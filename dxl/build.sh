#!/bin/sh
# Cross-build for the board (aarch64); ./build.sh runs in the dist CI and on
# dev hosts. Static musl keeps the binary independent of the build glibc.
set -e
cd "$(dirname "$0")"
cargo build --release --target aarch64-unknown-linux-musl
cp target/aarch64-unknown-linux-musl/release/dxl dxl
