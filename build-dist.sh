#!/bin/sh
# Build and pack the deployable dist tarball: one directory per tool, laid
# out exactly as it will land under /opt/microduck-test/ on the board.
#
# Per-tool contract:
#   <tool>/install.sh <dir>  optional dist entry; cross-builds (via
#                            ./build.sh when needed) and installs the
#                            runtime payload into <dir>
#   (no install.sh)          the whole directory ships as-is
#                            (script-only tools)
# Adding a tool = adding a directory that follows this contract; neither
# this script nor the Armbian extension need changes.
set -e
cd "$(dirname "$0")"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

for d in */; do
	name="${d%/}"
	git ls-files -- "$name" | grep -q . || continue  # skip untracked local dirs
	if [ -f "$d/install.sh" ]; then
		( cd "$d" && sh ./install.sh "$stage/$name" )
	else
		mkdir -p "$stage/$name"
		( cd "$d" && git ls-files -z | xargs -0 cp --parents -t "$stage/$name" )
	fi
done
install -m 644 microduck-test-tools.sh "$stage/microduck-test-tools.sh"

out=test-tools-dist.tar.gz
tar -C "$stage" -czf "$out" .
echo "packed $out"
