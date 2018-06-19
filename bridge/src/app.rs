use std::path::{Path, PathBuf};
use tokio_core::reactor::{Handle};
use tokio_timer::{self, Timer};
use error::{Error, ErrorKind};
use config::{Config, RpcUrl, RpcUrlKind};
use contracts::{home, foreign};
use web3::{Transport, transports::http::Http, error::Error as Web3Error};
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
	pub home_url: RpcUrlKind,
	pub foreign: T,
	pub foreign_url: RpcUrlKind,
}

impl Connections<Http>  {
	/// Returns new home and foreign HTTP transport connections, falling back
	/// to failover urls if necessary.
	pub fn new_http(handle: &Handle, home_primary: &RpcUrl, home_failover: Option<&RpcUrl>,
			home_concurrent_connections: usize, foreign_primary: &RpcUrl,
			foreign_failover: Option<&RpcUrl>, foreign_concurrent_connections: usize)
			-> Result<Self, Error> {
		// Attempts to connect to either a primary or failover url, returning
		// the transport and the url upon success.
		fn connect(handle: &Handle, url_primary: &RpcUrl, url_failover: Option<&RpcUrl>,
				concurrent_connections: usize) -> Result<(Http, RpcUrlKind), Web3Error> {
			match Http::with_event_loop(&url_primary.to_string(), handle, concurrent_connections) {
				Ok(t) => Ok((t, RpcUrlKind::Primary(url_primary.clone()))),
				Err(err) => match url_failover {
					Some(fo) => {
						Http::with_event_loop(&fo.to_string(), handle, concurrent_connections)
							.map(|h| (h, RpcUrlKind::Failover(fo.clone())))
					},
					None => Err(err),
				},
			}
		}

		let (home, home_url) = connect(handle, home_primary, home_failover, home_concurrent_connections)
			.map_err(|err| ErrorKind::HomeRpcConnection(err))?;
		let (foreign, foreign_url) = connect(handle, foreign_primary, foreign_failover, foreign_concurrent_connections)
			.map_err(|err| ErrorKind::ForeignRpcConnection(err))?;

		Ok(Connections {
			home,
			home_url,
			foreign,
			foreign_url,
		})
	}
}

/// Contains references to the fields of a `Connection`.
pub struct ConnectionsRef<'u, T> where T: Transport {
	pub home: T,
	pub home_url: &'u RpcUrlKind,
	pub foreign: T,
	pub foreign_url: &'u RpcUrlKind,
}

impl<'u, T: Transport> ConnectionsRef<'u, T> {
	pub fn as_ref(&'u self) -> ConnectionsRef<'u, &T> {
		ConnectionsRef {
			home: &self.home,
			home_url: &self.home_url,
			foreign: &self.foreign,
			foreign_url: &self.foreign_url,
		}
	}
}

impl App<Http> {
	pub fn new_http<P: AsRef<Path>>(config: Config, database_path: P, handle: &Handle,
			running: Arc<AtomicBool>) -> Result<Self, Error> {
		let connections = Connections::new_http(
			handle,
			&config.home.primary_rpc,
			config.home.failover_rpc.as_ref(),
			config.home.concurrent_http_requests,
			&config.foreign.primary_rpc,
			config.foreign.failover_rpc.as_ref(),
			config.foreign.concurrent_http_requests,
		)?;

		let keystore = EthStore::open(Box::new(RootDiskDirectory::at(&config.keystore)))
			.map_err(|e| ErrorKind::KeyStore(e))?;

		let keystore = AccountProvider::new(Box::new(keystore), AccountProviderSettings {
			enable_hardware_wallets: false,
			hardware_wallet_classic_key: false,
			unlock_keep_secret: true,
			blacklisted_accounts: vec![],
		});
		keystore.unlock_account_permanently(config.home.account, config.home.password()?)
			.map_err(|e| ErrorKind::AccountError(e))?;
		keystore.unlock_account_permanently(config.foreign.account, config.foreign.password()?)
			.map_err(|e| ErrorKind::AccountError(e))?;

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

