use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use ethcore::account_provider::{AccountProvider, AccountProviderSettings};
use ethcore::ethstore::EthStore;
use ethcore::ethstore::accounts_dir::RootDiskDirectory;
use tokio_core::reactor::Handle;
use tokio_timer::{self, Timer};
use web3::Transport;
use web3::transports::http::Http;

use error::{Error, ErrorKind, ResultExt};
use config::Config;
use contracts::foreign::ForeignBridge;
use contracts::home::HomeBridge;

/// Holds the HTTP connections to the home and foreign Ethereum nodes.
pub struct Connections<T: Transport> {
	pub home: T,
	pub foreign: T
}

impl Connections<Http>  {
	pub fn new_http(handle: &Handle, home: &str, foreign: &str) -> Result<Self, Error> {
	    let home = Http::with_event_loop(home, handle, 1)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(||"Cannot connect to home node rpc")?;
	
		let foreign = Http::with_event_loop(foreign, handle, 1)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(||"Cannot connect to foreign node rpc")?;

		Ok(Connections { home, foreign })
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

pub struct App<T> where T: Transport {
	pub config: Config,
	pub database_path: PathBuf,
	pub connections: Connections<T>,
	pub home_bridge: HomeBridge,
	pub foreign_bridge: ForeignBridge,
	pub timer: Timer,
	pub running: Arc<AtomicBool>,
	pub keystore: AccountProvider
}

impl App<Http> {
	pub fn new_http<P: AsRef<Path>>(config: Config, database_path: P, handle: &Handle, running: Arc<AtomicBool>) -> Result<Self, Error> {
		let database_path = database_path.as_ref().to_path_buf();
		
		let connections = {
			let home_url = format!("{}:{}", config.home.rpc_host, config.home.rpc_port);
			let foreign_url = format!("{}:{}", config.foreign.rpc_host, config.foreign.rpc_port);
			Connections::new_http(handle, &home_url, &foreign_url)?
		};

		let home_bridge = HomeBridge::default();
		let foreign_bridge = ForeignBridge::default();

		// It is important to build a timer with a max timeout that can
		// accommodate the longest timeout requested, otherwise it will
		// result in a bizarrely inadequate behaviour of timing out nearly
		// immediately.
		let timer = {
			let max_timeout = config.home.request_timeout.max(config.foreign.request_timeout);
			tokio_timer::wheel().max_timeout(max_timeout)
				.tick_duration(Duration::from_millis(100))
				.num_slots((max_timeout.as_secs() as usize * 10).next_power_of_two())
				.build()
		};

		let keystore = {
			let dir = Box::new(RootDiskDirectory::at(&config.keystore));
			let eth_store = EthStore::open(dir).map_err(|e| ErrorKind::KeyStore(e))?;
			let settings = AccountProviderSettings {
				enable_hardware_wallets: false,
				hardware_wallet_classic_key: false,
				unlock_keep_secret: true,
				blacklisted_accounts: vec![]
			};
			AccountProvider::new(Box::new(eth_store), settings)
		};

		keystore.unlock_account_permanently(config.home.account, config.home.password()?)
			.map_err(|e| ErrorKind::AccountError(e))?;
			
		keystore.unlock_account_permanently(config.foreign.account, config.foreign.password()?)
			.map_err(|e| ErrorKind::AccountError(e))?;

		let app = App {
			config, database_path, connections,
			home_bridge, foreign_bridge,
			timer, running, keystore
		};

		Ok(app)
	}
}
