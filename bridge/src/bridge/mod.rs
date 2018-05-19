mod deploy;
mod balance;
mod chain_id;
pub mod nonce;
mod deposit_relay;
mod withdraw_confirm;
mod withdraw_relay;

use std::fs;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use futures::{Stream, Poll, Async};
use web3::Transport;
use web3::types::{U256, Address};
use app::App;
use database::Database;
use error::{Error, ErrorKind, Result};

pub use self::deploy::{Deploy, Deployed, create_deploy};
pub use self::balance::{BalanceCheck, create_balance_check};
pub use self::chain_id::{ChainIdRetrieval, create_chain_id_retrieval};
pub use self::deposit_relay::{DepositRelay, create_deposit_relay};
pub use self::withdraw_relay::{WithdrawRelay, create_withdraw_relay};
pub use self::withdraw_confirm::{WithdrawConfirm, create_withdraw_confirm};

/// Last block checked by the bridge components.
#[derive(Clone, Copy)]
pub enum BridgeChecked {
	DepositRelay(u64),
	WithdrawRelay((u64, u32)),
	WithdrawConfirm(u64),
}

pub trait BridgeBackend {
	fn save(&mut self, checks: Vec<BridgeChecked>) -> Result<()>;
}

pub struct FileBackend {
	path: PathBuf,
	database: Database,
}

impl BridgeBackend for FileBackend {
	fn save(&mut self, checks: Vec<BridgeChecked>) -> Result<()> {
		for check in checks {
			match check {
				BridgeChecked::DepositRelay(n) => {
					self.database.checked_deposit_relay = n;
				},
				BridgeChecked::WithdrawRelay((n, sigs)) => {
					self.database.checked_withdraw_relay = n;
					self.database.withdraw_relay_required_signatures = Some(sigs);
				},
				BridgeChecked::WithdrawConfirm(n) => {
					self.database.checked_withdraw_confirm = n;
				},
			}
		}

		let file = fs::OpenOptions::new()
			.write(true)
			.create(true)
			.open(&self.path)?;

		self.database.save(file)
	}
 }

enum BridgeStatus {
	Wait,
	NextItem(Option<()>),
}

/// Creates new bridge.
pub fn create_bridge<T: Transport + Clone>(app: Arc<App<T>>, init: &Database, home_chain_id: u64, foreign_chain_id: u64, foreign_validator_contract: Address) -> Bridge<T, FileBackend> {
	let backend = FileBackend {
		path: app.database_path.clone(),
		database: init.clone(),
	};

	create_bridge_backed_by(app, init, backend, home_chain_id, foreign_chain_id, foreign_validator_contract)
}

/// Creates new bridge writing to custom backend.
pub fn create_bridge_backed_by<T: Transport + Clone, F: BridgeBackend>(app: Arc<App<T>>, init: &Database, backend: F, home_chain_id: u64, foreign_chain_id: u64,
																	   foreign_validator_contract: Address) -> Bridge<T, F> {
	let home_balance = Arc::new(RwLock::new(None));
	let foreign_balance = Arc::new(RwLock::new(None));
	Bridge {
		foreign_balance_check: create_balance_check(app.clone(), app.connections.foreign.clone(), app.config.foreign.clone()),
		home_balance_check: create_balance_check(app.clone(), app.connections.home.clone(), app.config.home.clone()),
		foreign_balance: foreign_balance.clone(),
		home_balance: home_balance.clone(),
		deposit_relay: create_deposit_relay(app.clone(), init, foreign_balance.clone(), foreign_chain_id),
		withdraw_relay: create_withdraw_relay(app.clone(), init, home_balance.clone(), home_chain_id, foreign_validator_contract),
		withdraw_confirm: create_withdraw_confirm(app.clone(), init, foreign_balance.clone(), foreign_chain_id),
		state: BridgeStatus::Wait,
		backend,
		running: app.running.clone(),
	}
}

pub struct Bridge<T: Transport, F> {
	home_balance_check: BalanceCheck<T>,
	foreign_balance_check: BalanceCheck<T>,
	home_balance: Arc<RwLock<Option<U256>>>,
	foreign_balance: Arc<RwLock<Option<U256>>>,
	deposit_relay: DepositRelay<T>,
	withdraw_relay: WithdrawRelay<T>,
	withdraw_confirm: WithdrawConfirm<T>,
	state: BridgeStatus,
	backend: F,
	running: Arc<AtomicBool>,
}

use std::sync::atomic::{AtomicBool, Ordering};

impl<T: Transport, F: BridgeBackend> Bridge<T, F> {
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

}

impl<T: Transport, F: BridgeBackend> Stream for Bridge<T, F> {
	type Item = ();
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				BridgeStatus::Wait => {
					if !self.running.load(Ordering::SeqCst) {
						return Err(ErrorKind::ShutdownRequested.into())
					}

					// Intended to be used upon startup
					let balance_is_absent = {
						let mut home_balance = self.home_balance.read().unwrap();
						let mut foreign_balance = self.foreign_balance.read().unwrap();
						home_balance.is_none() || foreign_balance.is_none()
					};
					if balance_is_absent {
						match self.check_balances()? {
							Async::NotReady => return Ok(Async::NotReady),
							_ => (),
						}
					}

					let d_relay = try_bridge!(self.deposit_relay.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "deposit_relay")))
						.map(BridgeChecked::DepositRelay);

					if d_relay.is_some() {
						self.check_balances()?;
					}


					let w_relay = try_bridge!(self.withdraw_relay.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "withdraw_relay"))).
						map(BridgeChecked::WithdrawRelay);

					if w_relay.is_some() {
						self.check_balances()?;
					}


					let w_confirm = try_bridge!(self.withdraw_confirm.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "withdraw_confirm"))).
						map(BridgeChecked::WithdrawConfirm);

					if w_confirm.is_some() {
						self.check_balances()?;
					}

					let result: Vec<_> = [d_relay, w_relay, w_confirm]
						.into_iter()
						.filter_map(|c| *c)
						.collect();

					if result.is_empty() {
						return Ok(Async::NotReady);
					} else {
						self.backend.save(result)?;
						BridgeStatus::NextItem(Some(()))
					}
				},
				BridgeStatus::NextItem(ref mut v) => match v.take() {
					None => BridgeStatus::Wait,
					some => return Ok(some.into()),
				},
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
	use super::{BridgeBackend, FileBackend, BridgeChecked};

	#[test]
	fn test_file_backend() {
		let tempdir = TempDir::new("test_file_backend").unwrap();
		let mut path = tempdir.path().to_owned();
		path.push("db");
		let mut backend = FileBackend {
			path: path.clone(),
			database: Database::default(),
		};

		backend.save(vec![BridgeChecked::DepositRelay(1)]).unwrap();
		assert_eq!(1, backend.database.checked_deposit_relay);
		assert_eq!(0, backend.database.checked_withdraw_confirm);
		assert_eq!(0, backend.database.checked_withdraw_relay);
		backend.save(vec![BridgeChecked::DepositRelay(2), BridgeChecked::WithdrawConfirm(3), BridgeChecked::WithdrawRelay((2, 1))]).unwrap();
		assert_eq!(2, backend.database.checked_deposit_relay);
		assert_eq!(3, backend.database.checked_withdraw_confirm);
		assert_eq!(2, backend.database.checked_withdraw_relay);
		assert_eq!(1, backend.database.withdraw_relay_required_signatures.unwrap());

		let loaded = Database::load(path).unwrap();
		assert_eq!(backend.database, loaded);
	}
}
