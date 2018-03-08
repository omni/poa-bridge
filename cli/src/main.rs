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

use std::{env, fs};
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

fn main() {
	let _ = env_logger::init();
	let result = execute(env::args());

	match result {
		Ok(s) => println!("{}", s),
		Err(err) => print_err(err),
	}
}

fn print_err(err: Error) {
	let message = err.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n\nCaused by:\n  ");
	println!("{}", message);
}

fn execute<S, I>(command: I) -> Result<String, Error> where I: IntoIterator<Item=S>, S: AsRef<str> {
	info!(target: "bridge", "Parsing cli arguments");
	let args: Args = Docopt::new(USAGE)
		.and_then(|d| d.argv(command).deserialize()).map_err(|e| e.to_string())?;

	info!(target: "bridge", "Loading config");
	let config = Config::load(args.arg_config)?;

	info!(target: "bridge", "Starting event loop");
	let mut event_loop = Core::new().unwrap();

	info!(target: "bridge", "Establishing ipc connection");
	let app = loop {
		match App::new_ipc(config.clone(), &args.arg_database, &event_loop.handle()) {
			Ok(app) => break app,
			Err(e) => {
				warn!("Can't establish an IPC connection, will attempt to reconnect: {:?}", e);
				::std::thread::sleep(::std::time::Duration::from_secs(1));
			},
		}
	};
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
	let mut result = event_loop.run(bridge);
	loop {
		result = match &result {
			&Err(Error(ErrorKind::Web3(web3::error::Error(web3::error::ErrorKind::Io(ref e), _)), _)) if e.kind() == ::std::io::ErrorKind::BrokenPipe => {
			    warn!("Connection to a node has been severed, attempting to reconnect");
				let app = match App::new_ipc(config.clone(), &args.arg_database, &event_loop.handle()) {
					Ok(app) => {
						warn!("Connection has been re-established, restarting");
						app
					},
					_ => {
						::std::thread::sleep(::std::time::Duration::from_secs(1));
						continue
					},
				};
				let app_ref = Arc::new(app.as_ref());
				let bridge = create_bridge(app_ref.clone(), &database).and_then(|_| future::ok(true)).collect();
				event_loop.run(bridge)
			},
			&Err(ref e) => {
				warn!("Bridge is down with {}, attempting to restart", e);
				let bridge = create_bridge(app_ref.clone(), &database).and_then(|_| future::ok(true)).collect();
				event_loop.run(bridge)
			},
			&Ok(_) => break,
		};
	}

	Ok("Done".into())
}


#[cfg(test)]
mod tests {
}
