// Link against the closed-source VQE libs. Vendored under vendor/ so CI
// builds without the SDK; set SDK=... to use a full SDK checkout instead.
use std::path::PathBuf;

fn main() {
	let vendored = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("vendor/lib64");
	let lib = if vendored.exists() {
		vendored
	} else {
		let sdk = std::env::var_os("SDK")
			.map(PathBuf::from)
			.unwrap_or_else(|| {
				let home = std::env::var_os("HOME").expect("HOME");
				PathBuf::from(home).join("work/rk/sdk-pull/rk3566_rk3568-linux6.1")
			});
		sdk.join("external/common_algorithm/audio/rkaudio_algorithms/lib64")
	};
	println!("cargo:rustc-link-search=native={}", lib.display());
	println!("cargo:rustc-link-lib=dylib=rkaudio_vqe");
	println!("cargo:rustc-link-lib=dylib=rkaudio_common");
	println!("cargo:rerun-if-env-changed=SDK");
}
