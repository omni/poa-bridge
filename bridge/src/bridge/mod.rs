mod balance;
mod chain_id;
pub mod nonce;
mod deposit_confirm;
mod deposit_relay;
mod withdraw_relay;
mod gas_price;

use std::fs;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use futures::{Stream, Poll, Async};
use web3::Transport;
use web3::types::U256;
use app::App;
use database::Database;
use error::{Error, ErrorKind};
use tokio_core::reactor::Handle;

pub use self::balance::{BalanceCheck, create_balance_check};
pub use self::chain_id::{ChainIdRetrieval, create_chain_id_retrieval};
pub use self::deposit_confirm::create_deposit_confirm;
pub use self::deposit_relay::create_deposit_relay;
pub use self::withdraw_relay::create_withdraw_relay;
pub use self::gas_price::StandardGasPriceStream;

/// Last block checked by the bridge components.
#[derive(Clone, Copy, Debug)]
pub enum BridgeChecked {
	DepositConfirm(u64),
	DepositRelay(u64),
	WithdrawRelay(u64),
}

pub struct Bridge<ES: Stream<Item = BridgeChecked>> {
	path: PathBuf,
	database: Database,
	event_stream: ES,
}

impl<ES: Stream<Item = BridgeChecked, Error = Error>> Stream for Bridge<ES> {
	type Item = ();
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		let check = try_stream!(self.event_stream.poll());
		match check {
			BridgeChecked::DepositConfirm(n) => {
				self.database.checked_deposit_confirm = n;
			},
			BridgeChecked::DepositRelay(n) => {
				self.database.checked_deposit_relay = n;
			},
			BridgeChecked::WithdrawRelay(n) => {
				self.database.checked_withdraw_relay = n;
			},
		}
		let file = fs::OpenOptions::new()
			.write(true)
			.create(true)
			.open(&self.path)?;

		self.database.save(file)?;
		Ok(Async::Ready(Some(())))
	}
}


/// Creates new bridge.
pub fn create_bridge<'a, T: Transport + 'a + Clone>(app: Arc<App<T>>, init: &Database, handle: &Handle, home_chain_id: u64, foreign_chain_id: u64) -> Bridge<BridgeEventStream<'a, T>> {
	Bridge {
		path: app.database_path.clone(),
		database: init.clone(),
		event_stream: create_bridge_event_stream(app, init, handle, home_chain_id, foreign_chain_id),
	}
}

/// Creates new bridge writing to custom backend.
pub fn create_bridge_event_stream<'a, T: Transport + 'a + Clone>(app: Arc<App<T>>, init: &Database, handle: &Handle, home_chain_id: u64, foreign_chain_id: u64) -> BridgeEventStream<'a, T> {
	let home_balance = Arc::new(RwLock::new(None));
	let foreign_balance = Arc::new(RwLock::new(None));

	let home_gas_stream = if app.config.home.gas_price_oracle_url.is_some() {
		let stream = StandardGasPriceStream::new(&app.config.home, handle, &app.timer);
		Some(stream)
	} else {
		None
	};

	let foreign_gas_stream = if app.config.foreign.gas_price_oracle_url.is_some() {
		let stream = StandardGasPriceStream::new(&app.config.foreign, handle, &app.timer);
		Some(stream)
	} else {
		None
	};

	let home_gas_price = Arc::new(RwLock::new(app.config.home.default_gas_price));
	let foreign_gas_price = Arc::new(RwLock::new(app.config.foreign.default_gas_price));

	let deposit_confirm = create_deposit_confirm(app.clone(), init, home_balance.clone(), home_chain_id, home_gas_price.clone())
		.map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "deposit_confirm").into());
	let deposit_relay = create_deposit_relay(app.clone(), init, foreign_balance.clone(), foreign_chain_id, foreign_gas_price.clone())
		.map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "deposit_relay").into());
	let withdraw_relay = create_withdraw_relay(app.clone(), init, home_balance.clone(), home_chain_id, home_gas_price.clone())
		.map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "withdraw_relay").into());
	
	let bridge = Box::new(deposit_confirm.select(deposit_relay).select(withdraw_relay));

	BridgeEventStream {
		foreign_balance_check: create_balance_check(app.clone(), app.connections.foreign.clone(), app.config.foreign.clone()),
		home_balance_check: create_balance_check(app.clone(), app.connections.home.clone(), app.config.home.clone()),
		foreign_balance: foreign_balance.clone(),
		home_balance: home_balance.clone(),
		bridge,
		state: BridgeStatus::Init,
		running: app.running.clone(),
		home_gas_stream,
		foreign_gas_stream,
		home_gas_price,
		foreign_gas_price,
	}
}

enum BridgeStatus {
	Init,
	Wait,
	NextItem(Option<BridgeChecked>),
}

pub struct BridgeEventStream<'a, T: Transport + 'a> {
	home_balance_check: BalanceCheck<T>,
	foreign_balance_check: BalanceCheck<T>,
	home_balance: Arc<RwLock<Option<U256>>>,
	foreign_balance: Arc<RwLock<Option<U256>>>,
	bridge: Box<Stream<Item = BridgeChecked, Error = Error> + 'a>,
	state: BridgeStatus,
	running: Arc<AtomicBool>,
	home_gas_stream: Option<StandardGasPriceStream>,
	foreign_gas_stream: Option<StandardGasPriceStream>,
	home_gas_price: Arc<RwLock<u64>>,
	foreign_gas_price: Arc<RwLock<u64>>,
}

use std::sync::atomic::{AtomicBool, Ordering};

impl<'a, T: Transport + 'a> BridgeEventStream<'a, T> {
	fn check_balances(&mut self) -> Poll<Option<()>, Error> {
		let mut home_balance = self.home_balance.write().unwrap();
		let mut foreign_balance = self.foreign_balance.write().unwrap();
		let home_balance_known = home_balance.is_some();
		let foreign_balance_known = foreign_balance.is_some();
		*home_balance = try_bridge!(self.home_balance_check.poll()).or(*home_balance);
		*foreign_balance = try_bridge!(self.foreign_balance_check.poll()).or(*foreign_balance);
		if !home_balance_known && home_balance.is_some() {
				info!("Retrieved home contract balance");
		}
		if !foreign_balance_known && foreign_balance.is_some() {
				info!("Retrieved foreign contract balance");
		}
		if home_balance.is_none() || foreign_balance.is_none() {
			Ok(Async::NotReady)
		} else {
			Ok(Async::Ready(None))
		}
	}

	fn get_gas_prices(&mut self) -> Poll<Option<()>, Error> {
		if let Some(ref mut home_gas_stream) = self.home_gas_stream {
			let mut home_price = self.home_gas_price.write().unwrap();
			*home_price = try_bridge!(home_gas_stream.poll()).unwrap_or(*home_price);
		}

		if let Some(ref mut foreign_gas_stream) = self.foreign_gas_stream {
			let mut foreign_price = self.foreign_gas_price.write().unwrap();
			*foreign_price = try_bridge!(foreign_gas_stream.poll()).unwrap_or(*foreign_price);
		}

		Ok(Async::Ready(None))
	}
}

impl<'a, T: Transport + 'a> Stream for BridgeEventStream<'a, T> {
	type Item = BridgeChecked;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				BridgeStatus::Init => {
					match self.check_balances()? {
						Async::NotReady => return Ok(Async::NotReady),
						_ => (),
					}
					BridgeStatus::Wait
				},
				BridgeStatus::Wait => {
					if !self.running.load(Ordering::SeqCst) {
						return Err(ErrorKind::ShutdownRequested.into())
					}

					let _ = self.get_gas_prices();

					let item = try_stream!(self.bridge.poll());
					BridgeStatus::NextItem(Some(item))
				},
				BridgeStatus::NextItem(ref mut v) => match v.take() {
					None => BridgeStatus::Init,
					some => {
						return Ok(some.into());
					}
				}
			};

			self.state = next_state;
		}
	}
}

#[cfg(test)]
mod tests {
	extern crate tempdir;
	use self::tempdir::TempDir;
	use database::Database;
	use super::{Bridge, BridgeChecked};
	use error::Error;
	use tokio_core::reactor::Core;
	use futures::{Stream, stream};

	#[test]
	fn test_database_updates() {
		let tempdir = TempDir::new("test_file_backend").unwrap();
		let mut path = tempdir.path().to_owned();
		path.push("db");

		let bridge = Bridge {
			path: path.clone(),
			database: Database::default(),
			event_stream: stream::iter_ok::<_, Error>(vec![BridgeChecked::DepositRelay(1)]),
		};

		let mut event_loop = Core::new().unwrap();
		let _ = event_loop.run(bridge.collect());

		let db = Database::load(&path).unwrap();
		assert_eq!(0, db.checked_deposit_confirm);
		assert_eq!(1, db.checked_deposit_relay);
		assert_eq!(0, db.checked_withdraw_relay);

		let bridge = Bridge {
			path: path.clone(),
			database: Database::default(),
			event_stream: stream::iter_ok::<_, Error>(vec![BridgeChecked::DepositConfirm(1), BridgeChecked::DepositRelay(2), BridgeChecked::WithdrawRelay(3)]),
		};

		let mut event_loop = Core::new().unwrap();
		let _ = event_loop.run(bridge.collect());

		let db = Database::load(&path).unwrap();
		assert_eq!(1, db.checked_deposit_confirm);
		assert_eq!(2, db.checked_deposit_relay);
		assert_eq!(3, db.checked_withdraw_relay);
	}
}
