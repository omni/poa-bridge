extern crate web3;

use std::path::{Path, PathBuf};
use tokio_core::reactor::{Handle};
use tokio_timer::Timer;
use web3::Transport;
use web3::transports::ipc::Ipc;
use web3::futures::Future; // For RPC Support
use error::{Error, ResultExt, ErrorKind};
use config::Config;
use contracts::{home, foreign};

pub struct App<T> where T: Transport {
	pub config: Config,
	pub database_path: PathBuf,
	pub connections: Connections<T>,
	pub home_bridge: home::HomeBridge,
	pub foreign_bridge: foreign::ForeignBridge,
	pub timer: Timer,
}

pub struct Connections<T> where T: Transport {
	pub home: T,
	pub foreign: T,
}

impl Connections<Ipc> {
	pub fn new_ipc<P: AsRef<Path>>(handle: &Handle, home: P, foreign: P) -> Result<Self, Error> {
		let home = Ipc::with_event_loop(home, handle)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(|| "Cannot connect to home node ipc")?;
		let foreign = Ipc::with_event_loop(foreign, handle)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(|| "Cannot connect to foreign node ipc")?;

		let result = Connections {
			home,
			foreign,
		};
		Ok(result)
	}
	pub fn new_rpc<P: AsRef<Path>>(handle: &Handle, home: P, foreign: P) -> Result<Self, Error> {
		let (_eloop, http) = web3::transports::Http::new("http://localhost:8545").unwrap();
		let web3 = web3::Web3::new(http);
		// let accounts = web3.eth().accounts().wait().unwrap();
		// println!("Accounts: {:?}", accounts);

		let home = web3::Web3::new(http);

		let foreign = web3::Web3::new(http);

		let result = Connections {
			home,
			foreign,
		};
		Ok(result)
	}
}

impl<T: Transport> Connections<T> {
	pub fn as_ref(&self) -> Connections<&T> {
		Connections {
			home: &self.home,
			foreign: &self.foreign,
		}
	}
}

impl App<Ipc> {
	pub fn new_ipc<P: AsRef<Path>>(config: Config, database_path: P, handle: &Handle) -> Result<Self, Error> {
		// @simonbdz I believe you want to insert the logic here. The description is confusing but I'm confident this is the correct logic for the parameters

		/*if config.home.ipc {
			// Assign result of new_ipc to connections
		}else if config.home.rpc_host && config.home.rpc_port {
			// Use new_rpc with host and port
		}else if config.home.rpc_host {
			// Same as above with default port
		}else {
			// Throw an error?
		}*/

		let connections = Connections::new_ipc(handle, &config.home.ipc, &config.foreign.ipc)?;
		let result = App {
			config,
			database_path: database_path.as_ref().to_path_buf(),
			connections,
			home_bridge: home::HomeBridge::default(),
			foreign_bridge: foreign::ForeignBridge::default(),
			timer: Timer::default(),
		};
		Ok(result)
	}
	// pub fn new_rpc<P: AsRef<Path>>(config: Config, database_path: P, handle: &Handle) -> Result<Self, Error> {
	// 	let result = App {
	// 		config,
	// 		database_path: database_path.as_ref().to_path_buf(),
	// 		connections,
	// 		home_bridge: home::HomeBridge::default(),
	// 		foreign_bridge: foreign::ForeignBridge::default(),
	// 		timer: Timer::default(),
	// 	};
	// 	Ok(accounts)
	// }
}

impl<T: Transport> App<T> {
	pub fn as_ref(&self) -> App<&T> {
		App {
			config: self.config.clone(),
			connections: self.connections.as_ref(),
			database_path: self.database_path.clone(),
			home_bridge: home::HomeBridge::default(),
			foreign_bridge: foreign::ForeignBridge::default(),
			timer: self.timer.clone(),
		}
	}
}
