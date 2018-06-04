use std::process::Command;

fn main() {
	let cmd = Command::new("git").args(&["describe", "--long", "--tags", "--always", "--dirty=-modified"]).output().unwrap();
	if cmd.status.success() {
		// if we're successful, use this as a version
		let ver = std::str::from_utf8(&cmd.stdout[1..]).unwrap().trim(); // drop "v" in the front
		println!("cargo:rustc-env={}={}", "CARGO_PKG_VERSION", ver);
	}
	// otherwise, whatever is specified in Cargo manifest
	println!("cargo:rerun-if-changed=nonexistentfile"); // always rerun build.rs
}
