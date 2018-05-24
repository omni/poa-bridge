extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate docopt;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate bridge;
extern crate ctrlc;
extern crate jsonrpc_core as rpc;

use std::{env, fs, io};
use std::sync::Arc;
use std::path::PathBuf;
use docopt::Docopt;
use futures::{Stream, future};
use tokio_core::reactor::Core;

use bridge::app::App;
use bridge::bridge::{create_bridge, create_deploy, create_chain_id_retrieval, Deployed};
use bridge::config::Config;
use bridge::error::{Error, ErrorKind};
use bridge::web3;

const ERR_UNKNOWN: i32 = 1;
const ERR_IO_ERROR: i32 = 2;
const ERR_SHUTDOWN_REQUESTED: i32 = 3;
const ERR_INSUFFICIENT_FUNDS: i32 = 4;
const ERR_GAS_TOO_LOW: i32 = 5;
const ERR_GAS_PRICE_TOO_LOW: i32 = 6;
const ERR_NONCE_REUSE: i32 = 7;
const ERR_CANNOT_CONNECT: i32 = 10;
const ERR_CONNECTION_LOST: i32 = 11;
const ERR_BRIDGE_CRASH: i32 = 12;
const ERR_RPC_ERROR: i32 = 20;

pub struct UserFacingError(i32, Error);

impl From<Error> for UserFacingError {
	fn from(err: Error) -> Self {
		UserFacingError(ERR_UNKNOWN, err)
	}
}

impl From<String> for UserFacingError {
	fn from(err: String) -> Self {
		UserFacingError(ERR_UNKNOWN, err.into())
	}
}


impl From<io::Error> for UserFacingError {
	fn from(err: io::Error) -> Self {
		UserFacingError(ERR_IO_ERROR, err.into())
	}
}


impl From<(i32, Error)> for UserFacingError {
	fn from((code, err): (i32, Error)) -> Self {
		UserFacingError(code, err)
	}
}


const USAGE: &'static str = r#"
POA-Ethereum bridge.
    Copyright 2017 Parity Technologies (UK) Limited
    Copyright 2018 POA Networks Ltd.

Usage:
    bridge --config <config> --database <database>
    bridge -h | --help

Options:
    -h, --help           Display help message and exit.
"#;

#[derive(Debug, Deserialize)]
pub struct Args {
	arg_config: PathBuf,
	arg_database: PathBuf,
}

use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
	let _ = env_logger::init();

	let running = Arc::new(AtomicBool::new(true));

	let r = running.clone();
	ctrlc::set_handler(move || {
		r.store(false, Ordering::SeqCst);
	}).expect("Error setting Ctrl-C handler");

	let result = execute(env::args(), running);

	match result {
		Ok(s) => println!("{}", s),
		Err(UserFacingError(code, err)) => {
			print_err(err);
			::std::process::exit(code);
		},
	}
}


fn print_err(err: Error) {
	let message = err.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n\nCaused by:\n  ");
	println!("{}", message);
}

fn execute<S, I>(command: I, running: Arc<AtomicBool>) -> Result<String, UserFacingError> where I: IntoIterator<Item=S>, S: AsRef<str> {
	info!(target: "bridge", "Parsing cli arguments");
	let args: Args = Docopt::new(USAGE)
		.and_then(|d| d.argv(command).deserialize()).map_err(|e| e.to_string())?;

	info!(target: "bridge", "Loading config");
	let config = Config::load(args.arg_config)?;

	info!(target: "bridge", "Starting event loop");
	let mut event_loop = Core::new().unwrap();
	let handle = event_loop.handle();

	info!(target: "bridge", "Home rpc host {}", config.clone().home.rpc_host);
	info!(target: "bridge", "Foreign rpc host {}", config.clone().foreign.rpc_host);

	info!(target: "bridge", "Establishing connection:");

	info!(target:"bridge", "  using RPC connection");
	let app = match App::new_http(config.clone(), &args.arg_database, &handle, running.clone()) {
		Ok(app) => app,
		Err(e) => {
			warn!("Can't establish an RPC connection: {:?}", e);
			return Err((ERR_CANNOT_CONNECT, e).into());
		},
	};

	let app = Arc::new(app);

	info!(target: "bridge", "Acquiring home & foreign chain ids");
	let home_chain_id = event_loop.run(create_chain_id_retrieval(app.clone(), app.connections.home.clone(), app.config.home.clone())).expect("can't retrieve home chain_id");
	let foreign_chain_id = event_loop.run(create_chain_id_retrieval(app.clone(), app.connections.foreign.clone(), app.config.foreign.clone())).expect("can't retrieve foreign chain_id");

	info!(target: "bridge", "Home chain ID: {} Foreign chain ID: {}", home_chain_id, foreign_chain_id);

	{
		use bridge::api;
		let mut home_nonce = app.config.home.info.nonce.write().unwrap();
		let mut foreign_nonce = app.config.foreign.info.nonce.write().unwrap();

		*home_nonce = event_loop.run(api::eth_get_transaction_count(app.connections.home.clone(), app.config.home.account, None)).expect("can't initialize home nonce");
		*foreign_nonce = event_loop.run(api::eth_get_transaction_count(app.connections.foreign.clone(), app.config.foreign.account, None)).expect("can't initialize foreign nonce");
	}

	#[cfg(feature = "deploy")]
	info!(target: "bridge", "Deploying contracts (if needed)");
	#[cfg(not(feature = "deploy"))]
	info!(target: "bridge", "Reading the database");

	let deployed = event_loop.run(create_deploy(app.clone(), home_chain_id, foreign_chain_id))?;

	let database = match deployed {
		Deployed::New(database) => {
			info!(target: "bridge", "Deployed new bridge contracts");
			info!(target: "bridge", "\n\n{}\n", database);
			database.save(fs::File::create(&app.database_path)?)?;
			database
		},
		Deployed::Existing(database) => {
			info!(target: "bridge", "Loaded database");
			database
		},
	};

	info!(target: "bridge", "Starting listening to events");
	let bridge = create_bridge(app.clone(), &database, &handle, home_chain_id, foreign_chain_id).and_then(|_| future::ok(true)).collect();
	let mut result = event_loop.run(bridge);
	loop {
		match result {
			Err(Error(ErrorKind::ContextualizedError(e, context), _)) => {
				error!("ERROR CONTEXT: {}", context);
				result = Err(*e);
				continue;
			}
			Err(Error(ErrorKind::Web3(web3::error::Error(web3::error::ErrorKind::Io(e), _)), _)) => {
				if e.kind() == ::std::io::ErrorKind::BrokenPipe {
					error!("Connection to a node has been severed");
					return Err((ERR_CONNECTION_LOST, e.into()).into());
				} else {
					error!("I/O error: {:?}", e);
					return Err((ERR_IO_ERROR, e.into()).into());
				}
			},
			Err(e @ Error(ErrorKind::ShutdownRequested, _)) => {
				info!("Shutdown requested, terminating");
				return Err((ERR_SHUTDOWN_REQUESTED, e.into()).into());
			},
			Err(e @ Error(ErrorKind::InsufficientFunds, _)) => {
				error!("Insufficient funds, terminating");
				return Err((ERR_INSUFFICIENT_FUNDS, e.into()).into());
			},
			Err(Error(ErrorKind::Web3(web3::error::Error(web3::error::ErrorKind::Rpc(e), _)), _)) => {
				if e.code == rpc::ErrorCode::ServerError(-32010) && e.message.starts_with("Insufficient funds") {
					error!("Insufficient funds, terminating");
					return Err((ERR_INSUFFICIENT_FUNDS, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				} else if e.code == rpc::ErrorCode::ServerError(-32010) && e.message.starts_with("Transaction gas is too low") {
					error!("Transaction gas is too low");
					return Err((ERR_GAS_TOO_LOW, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				} else if e.code == rpc::ErrorCode::ServerError(-32010) && e.message.starts_with("Transaction gas price is too low") {
					error!("Transaction gas price is too low");
					return Err((ERR_GAS_PRICE_TOO_LOW, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				} else if e.code == rpc::ErrorCode::ServerError(-32010) && e.message.starts_with("Transaction gas price is too low. There is another") {
					error!("Nonce reuse");
					return Err((ERR_NONCE_REUSE, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				} else if e.code == rpc::ErrorCode::ServerError(-32010) && e.message.starts_with("Transaction nonce is too low") {
					error!("Nonce reuse");
					return Err((ERR_NONCE_REUSE, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				} else {
					error!("RPC error {:?}", e);
					return Err((ERR_RPC_ERROR, ErrorKind::Web3(web3::error::ErrorKind::Rpc(e).into()).into()).into());
				}
			},
			Err(e) => {
				error!("Bridge crashed with {}", e);
				return Err((ERR_BRIDGE_CRASH, e).into());
			},
			Ok(_) => break,
		}
	}

	Ok("Done".into())
}


#[cfg(test)]
mod tests {
}
