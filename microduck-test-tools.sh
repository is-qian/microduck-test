# Armbian extension entry point for the test-tools dist tarball. The
# tarball (built by the test-tools CI via ./build-dist.sh) holds one
# directory per tool, laid out as it lands under /opt/microduck-test/.
#
# Dist layout contract:
#   <tool>/             copied to /opt/microduck-test/<tool>; if the dir
#                       contains install.sh it runs once in the chroot
#                       (deploy-only steps like firmware/package installs
#                       — never compilation; binaries ship prebuilt)

add_packages_to_image memtester libaio1

function post_install_kernel_debs__microduck_test_tools() {
	local tool name deb
	local dest_root="/opt/microduck-test"

	run_host_command_logged install -d "${SDCARD}${dest_root}"

	for tool in "${EXTENSION_DIR}"/*/; do
		name="$(basename "${tool}")"
		run_host_command_logged cp -r "${tool}" "${SDCARD}${dest_root}/${name}"
		if [ -f "${tool}/install.sh" ]; then
			chroot_sdcard sh "${dest_root}/${name}/install.sh"
		fi
	done
}
