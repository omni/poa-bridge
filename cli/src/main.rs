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

use std::{env, fs, io};
use std::sync::Arc;
use std::path::PathBuf;
use docopt::Docopt;
use futures::{Stream, future};
use tokio_core::reactor::Core;

use bridge::app::App;
use bridge::bridge::{create_bridge, create_deploy, Deployed};
use bridge::config::Config;
use bridge::error::{Error, ErrorKind};
use bridge::web3;

const ERR_UNKNOWN: i32 = 1;
const ERR_IO_ERROR: i32 = 2;
const ERR_SHUTDOWN_REQUESTED: i32 = 3;
const ERR_CANNOT_CONNECT: i32 = 10;
const ERR_CONNECTION_LOST: i32 = 11;
const ERR_BRIDGE_CRASH: i32 = 11;

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
Ethereum-Kovan bridge.
    Copyright 2017 Parity Technologies (UK) Limited

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

	info!(target: "bridge", "Home IPC file stem {:?}", config.clone().home.ipc.file_stem());

	info!(target: "bridge", "Home rpc host {}", config.clone().home.rpc_host);

/*
	// FIXME [edwardmack], figure out how to make this work.
	// I thought that since App<T> where T: Transport, and since Ipc and Http are Transport why doesn't this work
	let app = match config.clone().home.ipc.file_stem() {
		Some(_) =>
			{
				info!(target:"bridge", "USE IPC");
				match App::new_ipc(config.clone(), &args.arg_database, &event_loop.handle(), running) {
					Ok(app) => app,
					Err(e) => {
						warn!("Can't establish an IPC connection: {:?}", e);
						return Err((ERR_CANNOT_CONNECT, e).into());
					},
				}
			},
		None =>
			{
				info!(target: "bridge", "USE RPC");
				match App::new_http(config.clone(), &args.arg_database, &event_loop.handle(), running) {
					Ok(app) => app,
					Err(e) => {
						warn!("Can't establish an IPC connection: {:?}", e);
						return Err((ERR_CANNOT_CONNECT, e).into());
					},
				}
			},

	};
*/

	info!(target: "bridge", "Establishing connection:");

	info!(target: "bridge", "  using IPC connection");
	let app = match App::new_ipc(config.clone(), &args.arg_database, &event_loop.handle(), running) {
		Ok(app) => app,
		Err(e) => {
			warn!("Can't establish an IPC connection: {:?}", e);
			return Err((ERR_CANNOT_CONNECT, e).into());
		},
	};

/*
	info!(target:"bridge", "  using RPC connection");
	let app = match App::new_http(config.clone(), &args.arg_database, &event_loop.handle(), running) {
		Ok(app) => app,
		Err(e) => {
			warn!("Can't establish an RPC connection: {:?}", e);
			return Err((ERR_CANNOT_CONNECT, e).into());
		},
	};

*/
	let app_ref = Arc::new(app.as_ref());

	info!(target: "bridge", "Deploying contracts (if needed)");
	let deployed = event_loop.run(create_deploy(app_ref.clone()))?;

	let database = match deployed {
		Deployed::New(database) => {
			info!(target: "bridge", "Deployed new bridge contracts");
			info!(target: "bridge", "\n\n{}\n", database);
			database.save(fs::File::create(&app_ref.database_path)?)?;
			database
		},
		Deployed::Existing(database) => {
			info!(target: "bridge", "Loaded database");
			database
		},
	};

	info!(target: "bridge", "Starting listening to events");
	let bridge = create_bridge(app_ref.clone(), &database).and_then(|_| future::ok(true)).collect();
	let result = event_loop.run(bridge);
	match result {
			Err(Error(ErrorKind::Web3(web3::error::Error(web3::error::ErrorKind::Io(e), _)), _)) => {
				if e.kind() == ::std::io::ErrorKind::BrokenPipe {
					warn!("Connection to a node has been severed");
					return Err((ERR_CONNECTION_LOST, e.into()).into());
				} else {
					warn!("I/O error: {:?}", e);
					return Err((ERR_IO_ERROR, e.into()).into());
				}
			},
		    Err(e @ Error(ErrorKind::ShutdownRequested, _)) => {
				info!("Shutdown requested, terminating");
				return Err((ERR_SHUTDOWN_REQUESTED, e.into()).into());
			}
			Err(e) => {
				warn!("Bridge crashed with {}", e);
				return Err((ERR_BRIDGE_CRASH, e).into());
			},
			Ok(_) => (),
	}

	Ok("Done".into())
}


#[cfg(test)]
mod tests {
}
