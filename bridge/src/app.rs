use std::path::{Path, PathBuf};
use tokio_core::reactor::{Handle};
use tokio_timer::{self, Timer};
use web3::Transport;
use error::{Error, ResultExt, ErrorKind};
use config::Config;
use contracts::{home, foreign};
use web3::transports::http::Http;
use std::time::Duration;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ethcore::ethstore::{EthStore,accounts_dir::RootDiskDirectory};
use ethcore::account_provider::{AccountProvider, AccountProviderSettings};

pub struct App<T> where T: Transport {
	pub config: Config,
	pub database_path: PathBuf,
	pub connections: Connections<T>,
	pub home_bridge: home::HomeBridge,
	pub foreign_bridge: foreign::ForeignBridge,
	pub timer: Timer,
	pub running: Arc<AtomicBool>,
	pub keystore: AccountProvider,
}

pub struct Connections<T> where T: Transport {
	pub home: T,
	pub foreign: T,
}

impl Connections<Http>  {
	pub fn new_http(handle: &Handle, home: &str, foreign: &str) -> Result<Self, Error> {

	    let home = Http::with_event_loop(home, handle,1)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(||"Cannot connect to home node rpc")?;
		let foreign = Http::with_event_loop(foreign, handle, 1)
			.map_err(ErrorKind::Web3)
			.map_err(Error::from)
			.chain_err(||"Cannot connect to foreign node rpc")?;

		let result = Connections {
			home,
			foreign
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

impl App<Http> {
	pub fn new_http<P: AsRef<Path>>(config: Config, database_path: P, handle: &Handle, running: Arc<AtomicBool>) -> Result<Self, Error> {
		let home_url:String = format!("{}:{}", config.home.rpc_host, config.home.rpc_port);
		let foreign_url:String = format!("{}:{}", config.foreign.rpc_host, config.foreign.rpc_port);

		let connections = Connections::new_http(handle, home_url.as_ref(), foreign_url.as_ref())?;
		let keystore = EthStore::open(Box::new(RootDiskDirectory::at(&config.keystore))).map_err(|e| ErrorKind::KeyStore(e))?;

		let keystore = AccountProvider::new(Box::new(keystore), AccountProviderSettings {
			enable_hardware_wallets: false,
			hardware_wallet_classic_key: false,
			unlock_keep_secret: true,
			blacklisted_accounts: vec![],
		});
		keystore.unlock_account_permanently(config.home.account, config.home.password()?).map_err(|e| ErrorKind::AccountError(e))?;
		keystore.unlock_account_permanently(config.foreign.account, config.foreign.password()?).map_err(|e| ErrorKind::AccountError(e))?;

		let max_timeout = config.clone().home.request_timeout.max(config.clone().foreign.request_timeout);

		let result = App {
			config,
			database_path: database_path.as_ref().to_path_buf(),
			connections,
			home_bridge: home::HomeBridge::default(),
			foreign_bridge: foreign::ForeignBridge::default(),
			// it is important to build a timer with a max timeout that can accommodate the longest timeout requested,
			// otherwise it will result in a bizarrely inadequate behaviour of timing out nearly immediately
			timer: tokio_timer::wheel().max_timeout(max_timeout)
				.tick_duration(Duration::from_millis(100))
				.num_slots((max_timeout.as_secs() as usize * 10).next_power_of_two())
				.build(),
			running,
			keystore,
		};
		Ok(result)
	}
}
