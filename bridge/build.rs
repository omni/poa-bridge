use std::process::Command;
use std::str;

fn check_rustc_version() {
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        let version = str::from_utf8(&output.stdout).unwrap()
			.split(' ')
			.nth(1).unwrap();
        
		let mut split_version = version.split('.');
		let major_version: u8 = split_version.next().unwrap().parse().unwrap();
		let minor_version: u8 = split_version.next().unwrap().parse().unwrap();
		
		if major_version == 1 && minor_version < 26 {
		    panic!(
				"Invalid rustc version, `poa-bridge` requires \
				rustc >= 1.26, found version: {}",
				version
			);
		}
	}
}

fn main() {
	check_rustc_version();

	// rerun build script if bridge contract has changed.
	// without this cargo doesn't since the bridge contract
	// is outside the crate directories
	println!("cargo:rerun-if-changed=../contracts/bridge.sol");

	match Command::new("solc")
		.arg("--abi")
		.arg("--bin")
		.arg("--optimize")
		.arg("--output-dir").arg("../compiled_contracts")
		.arg("--overwrite")
		.arg("../contracts/bridge.sol")
		.status()
	{
		Ok(exit_status) => {
			if !exit_status.success() {
				if let Some(code) = exit_status.code() {
					panic!("`solc` exited with error exit status code `{}`", code);
				} else {
					panic!("`solc` exited because it was terminated by a signal");
				}
			}
		},
		Err(err) => {
			if let std::io::ErrorKind::NotFound = err.kind() {
				panic!("`solc` executable not found in `$PATH`. `solc` is required to compile the bridge contracts. please install it: https://solidity.readthedocs.io/en/develop/installing-solidity.html");
			} else {
				panic!("an error occurred when trying to spawn `solc`: {}", err);
			}
		}
	}
}
