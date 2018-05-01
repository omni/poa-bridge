/// spins up two parity nodes with the dev chain.
/// starts one bridge authority that connects the two.
/// does a deposit by sending ether to the HomeBridge.
/// asserts that the deposit got relayed to foreign chain.
/// does a withdraw by executing ForeignBridge.transferToHomeBridge.
/// asserts that the withdraw got relayed to home chain.

extern crate tempdir;
extern crate web3;
extern crate tokio_core;
extern crate bridge;
extern crate ethabi;
extern crate serde_json;
#[macro_use] extern crate ethabi_contract;
#[macro_use] extern crate ethabi_derive;

use_contract!(token, "Token", "../compiled_contracts/Token.abi");

use std::process::Command;
use std::time::Duration;
use std::thread;
use std::path::Path;

use tokio_core::reactor::Core;

use web3::transports::ipc::Ipc;
use web3::api::Namespace;
use web3::types::{Address, U256};

extern crate rustc_hex;
use rustc_hex::FromHex;

const TMP_PATH: &str = "tmp";

fn parity_home_command() -> Command {
	let mut command = Command::new("parity");
	command
		.arg("--base-path").arg(format!("{}/home", TMP_PATH))
		.arg("--chain").arg("dev")
		.arg("--ipc-path").arg("home.ipc")
		.arg("--logging").arg("rpc=trace")
		.arg("--jsonrpc-port").arg("8550")
		.arg("--jsonrpc-apis").arg("all")
		.arg("--port").arg("30310")
		.arg("--gasprice").arg("1")
		.arg("--reseal-min-period").arg("0")
		.arg("--no-ws")
		.arg("--no-dapps")
		.arg("--no-ui");
	command
}

fn parity_foreign_command() -> Command {
	let mut command = Command::new("parity");
	command
		.arg("--base-path").arg(format!("{}/foreign", TMP_PATH))
		.arg("--chain").arg("dev")
		.arg("--ipc-path").arg("foreign.ipc")
		.arg("--logging").arg("rpc=trace")
		.arg("--jsonrpc-port").arg("8551")
		.arg("--jsonrpc-apis").arg("all")
		.arg("--port").arg("30311")
		.arg("--gasprice").arg("1")
		.arg("--reseal-min-period").arg("0")
		.arg("--no-ws")
		.arg("--no-dapps")
		.arg("--no-ui");
	command
}

fn address_from_str(string: &'static str) -> web3::types::Address {
	web3::types::Address::from(&Address::from(string).0[..])
}

#[test]
fn test_insufficient_funds() {
	if Path::new(TMP_PATH).exists() {
		std::fs::remove_dir_all(TMP_PATH).expect("failed to remove tmp dir");
	}
	let _tmp_dir = tempdir::TempDir::new(TMP_PATH).expect("failed to create tmp dir");

	println!("\nbuild the bridge cli executable so we can run it later\n");
	assert!(Command::new("cargo")
		.env("RUST_BACKTRACE", "1")
		.current_dir("../cli")
		.args(&["build", "--features", "deploy"])
		.status()
		.expect("failed to build bridge cli")
		.success());

	// start a parity node that represents the home chain
	let mut parity_home = parity_home_command()
		.spawn()
		.expect("failed to spawn parity home node");

	// start a parity node that represents the foreign chain
	let mut parity_foreign = parity_foreign_command()
		.spawn()
		.expect("failed to spawn parity foreign node");

	// give the clients time to start up
	thread::sleep(Duration::from_millis(3000));

	// A address containing a lot of tokens (0x00a329c0648769a73afac7f9381e08fb43dbea72) should be
	// automatically added with a password being an empty string.
	// source: https://paritytech.github.io/wiki/Private-development-chain.html
	let user_address = "0x00a329c0648769a73afac7f9381e08fb43dbea72";

	let authority_address = "0x00bd138abd70e2f00903268f3db08f2d25677c9e";

	// create authority account on home
	let exit_status = Command::new("curl")
		.arg("--data").arg(r#"{"jsonrpc":"2.0","method":"parity_newAccountFromPhrase","params":["node0", ""],"id":0}"#)
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8550")
		.status()
		.expect("failed to create authority account on home");
	assert!(exit_status.success());
	// TODO [snd] assert that created address matches authority_address

	// create authority account on foreign
	let exit_status = Command::new("curl")
		.arg("--data").arg(r#"{"jsonrpc":"2.0","method":"parity_newAccountFromPhrase","params":["node0", ""],"id":0}"#)
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8551")
		.status()
		.expect("failed to create/unlock authority account on foreign");
	assert!(exit_status.success());

	// TODO [snd] assert that created address matches authority_address
	// give the operations time to complete
	thread::sleep(Duration::from_millis(5000));

	// kill the clients so we can restart them with the accounts unlocked
	parity_home.kill().unwrap();
	parity_foreign.kill().unwrap();

	// wait for clients to shut down
	thread::sleep(Duration::from_millis(5000));

	// start a parity node that represents the home chain with accounts unlocked
	let mut parity_home = parity_home_command()
		.arg("--unlock").arg(format!("{},{}", user_address, authority_address))
		.arg("--password").arg("password.txt")
		.spawn()
		.expect("failed to spawn parity home node");

	// start a parity node that represents the foreign chain with accounts unlocked
	let mut parity_foreign = parity_foreign_command()
		.arg("--unlock").arg(format!("{},{}", user_address, authority_address))
		.arg("--password").arg("password.txt")
		.spawn()
		.expect("failed to spawn parity foreign node");

	// give nodes time to start up
	thread::sleep(Duration::from_millis(10000));


	println!("\nfund foreign authority address\n");
	let exit_status = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{{
			"from": "{}",
			"to": "{}",
			"value": "{}"
		}}],"id":0}}"#, user_address, authority_address, "0xffffffff"))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8550")
		.status()
		.expect("failed to deposit into HomeBridge");
	assert!(exit_status.success());

	println!("\nfund foreign authority address (foreign)\n");
	let exit_status = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{{
			"from": "{}",
			"to": "{}",
			"value": "{}"
		}}],"id":0}}"#, user_address, authority_address, "0xffffffff"))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8551")
		.status()
		.expect("failed to deposit into HomeBridge");
	assert!(exit_status.success());


	// start bridge authority 1
	let mut bridge1 = Command::new("env")
		.arg("RUST_BACKTRACE=1")
		.arg("../target/debug/bridge")
		.env("RUST_LOG", "info")
		.arg("--config").arg("bridge_config_gas_price.toml")
		.arg("--database").arg("tmp/bridge1_db.txt")
		.spawn()
		.expect("failed to spawn bridge process");

	// give the bridge time to start up and deploy the contracts
	thread::sleep(Duration::from_millis(10000));

	let home_contract_address = "0xebd3944af37ccc6b67ff61239ac4fef229c8f69f";
	let foreign_contract_address = "0xebd3944af37ccc6b67ff61239ac4fef229c8f69f";

	// connect to foreign and home via IPC
	let mut event_loop = Core::new().unwrap();
	let foreign_transport = Ipc::with_event_loop("foreign.ipc", &event_loop.handle())
		.expect("failed to connect to foreign.ipc");
	let foreign = bridge::contracts::foreign::ForeignBridge::default();
	let foreign_eth = web3::api::Eth::new(foreign_transport);
	let home_transport = Ipc::with_event_loop("home.ipc", &event_loop.handle())
		.expect("failed to connect to home.ipc");
	let home_eth = web3::api::Eth::new(home_transport);

	// deploy the token
	println!("== deploy the token");
	let token_constructor = token::Token::default().constructor(include_str!("../../compiled_contracts/Token.bin").from_hex().unwrap());
	let future = foreign_eth.send_transaction(web3::types::TransactionRequest {
		from: address_from_str(authority_address),
		to: None,
		gas: None,
		gas_price: None,
		value: None,
		data: Some(token_constructor.into()),
		condition: None,
		nonce: None,
	});
	let tx = event_loop.run(future).unwrap();
	let future = foreign_eth.transaction_receipt(tx);
	let token_receipt = event_loop.run(future).unwrap();
	let token_addr = token_receipt.unwrap().contract_address.unwrap();

	// check token validity
    let is_token = token::Token::default().functions().is_token().input();
	let future = foreign_eth.call(web3::types::CallRequest {
		from: None,
		to: token_addr,
		gas: None,
		gas_price: None,
		value: None,
		data: Some(web3::types::Bytes(is_token)),
	}, None);

	event_loop.run(future).unwrap();

	// set the token
	println!("== set the token");
	let set_token = foreign.functions().set_token_address().input(token_addr);
	let future = foreign_eth.send_transaction(web3::types::TransactionRequest {
		from: address_from_str(authority_address),
		to: Some(address_from_str(foreign_contract_address)),
		gas: None,
		gas_price: None,
		value: None,
		data: Some(web3::types::Bytes(set_token)),
		condition: None,
		nonce: None,
	});
	event_loop.run(future).unwrap();

	// check that the token has been set correctly
	println!("== check token setup");
	let token = foreign.functions().erc20token().input();
	let future = foreign_eth.call(web3::types::CallRequest {
		from: None,
		to: address_from_str(foreign_contract_address),
		gas: None,
		gas_price: None,
		value: None,
		data: Some(web3::types::Bytes(token)),
	}, None);

	let response = event_loop.run(future).unwrap();
	assert_eq!(Address::from(&response.0.as_slice()[(response.0.len()-20)..]), token_addr);
	let erc20 = bridge::contracts::erc20::ERC20::default();

	// fund the contract
	println!("== set mint agent");
	let set_mint_agent = token::Token::default().functions().set_mint_agent().input(address_from_str(authority_address), true);
	let future = foreign_eth.send_transaction(web3::types::TransactionRequest {
		from: address_from_str(authority_address),
		to: Some(token_addr),
		gas: None,
		gas_price: None,
		value: None,
		data: Some(web3::types::Bytes(set_mint_agent)),
		condition: None,
		nonce: None,
	});
	event_loop.run(future).unwrap();

	println!("== fund contract through minting");
	let fund = token::Token::default().functions().mint().input(address_from_str(foreign_contract_address), ::std::u32::MAX);
	let future = foreign_eth.send_transaction(web3::types::TransactionRequest {
		from: address_from_str(authority_address),
		to: Some(token_addr),
		gas: None,
		gas_price: None,
		value: None,
		data: Some(web3::types::Bytes(fund)),
		condition: None,
		nonce: None,
	});
	event_loop.run(future).unwrap();

	println!("== check token supply");
	let supply_payload = token::Token::default().functions().total_supply().input();
	let supply_call = web3::types::CallRequest {
			from: None,
			to: token_addr,
			gas: None,
			gas_price: None,
			value: None,
			data: Some(web3::types::Bytes(supply_payload)),
	};
	assert!(!U256::from(event_loop.run(foreign_eth.call(supply_call, None)).unwrap().0.as_slice()).is_zero());


	println!("== check contract balance");
	let balance_payload = erc20.functions().balance_of().input(address_from_str(foreign_contract_address));
	let balance_call = web3::types::CallRequest {
			from: None,
			to: token_addr,
			gas: None,
			gas_price: None,
			value: None,
			data: Some(web3::types::Bytes(balance_payload)),
	};
	assert!(!U256::from(event_loop.run(foreign_eth.call(balance_call, None)).unwrap().0.as_slice()).is_zero());

	// TODO: remove the balance from the foreign bridge
	println!("\nchecking foreign balance");
	let balance_check = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"id": "balance", "jsonrpc":"2.0","method":"eth_getBalance","params":["{}"]}}"#, authority_address))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8551")
		.output()
		.expect("failed to get balance");
	let response: serde_json::Value = serde_json::from_reader(&balance_check.stdout[..]).unwrap();
	let balance = &response["result"];

	println!("\nbalance: {}", balance);

	println!("\nremoving funding from the foreign authority address\n");
	let exit_status = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{{
			"from": "{}",
			"to": "{}",
			"value": {},
            "gasPrice": "0x0"
		}}],"id":0}}"#, authority_address, "0x0000000000000000000000000000000000000000", balance))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8551")
		.status()
		.expect("failed to deposit into HomeBridge");
	assert!(exit_status.success());

	thread::sleep(Duration::from_millis(3000));

	println!("\nchecking foreign balance again");
	let balance_check = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"id": "balance", "jsonrpc":"2.0","method":"eth_getBalance","params":["{}"]}}"#, authority_address))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8551")
		.output()
		.expect("failed to get balance");
	let response: serde_json::Value = serde_json::from_reader(&balance_check.stdout[..]).unwrap();
	let balance = &response["result"];

	println!("\nbalance: {}", balance);

	assert_eq!(balance, &serde_json::Value::String("0x0".into()));


	println!("\nuser deposits ether into HomeBridge\n");
	// TODO [snd] use rpc client here instead of curl
	let exit_status = Command::new("curl")
		.arg("--data").arg(format!(r#"{{"jsonrpc":"2.0","method":"eth_sendTransaction","params":[{{
			"from": "{}",
			"to": "{}",
			"value": "0x186a0"
		}}],"id":0}}"#, user_address, home_contract_address))
		.arg("-H").arg("Content-Type: application/json")
		.arg("-X").arg("POST")
		.arg("localhost:8550")
		.status()
		.expect("failed to deposit into HomeBridge");
	assert!(exit_status.success());

	// TODO: bridge should exit
	let status = bridge1.wait().unwrap();
	assert_eq!(status.code().unwrap(), 4); // 4 stands for INSUFFICENT_FUNDS

	parity_home.kill().unwrap();
	parity_foreign.kill().unwrap();
}
