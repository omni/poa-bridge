use std::sync::Arc;
use futures::{Future, Poll};
#[cfg(feature = "deploy")]
use futures::future;
use web3::Transport;
#[cfg(feature = "deploy")]
use web3::types::U256;
use app::App;
use database::Database;
use error::{Error, ErrorKind};
#[cfg(feature = "deploy")]
use api;
#[cfg(feature = "deploy")]
use ethcore_transaction::{Transaction, Action};
#[cfg(feature = "deploy")]
use super::nonce::{NonceCheck,TransactionWithConfirmation};

pub enum Deployed {
	/// No existing database found. Deployed new contracts.
	New(Database),
	/// Reusing existing contracts.
	Existing(Database),
}

#[cfg(feature = "deploy")]
enum DeployState<T: Transport + Clone> {
	CheckIfNeeded,
	Deploying(future::Join<NonceCheck<T, TransactionWithConfirmation<T>>, NonceCheck<T, TransactionWithConfirmation<T>>>),
}

#[cfg(not(feature = "deploy"))]
enum DeployState {
	CheckIfNeeded,
}

#[allow(unused_variables)]
pub fn create_deploy<T: Transport + Clone>(app: Arc<App<T>>, home_chain_id: u64, foreign_chain_id: u64) -> Deploy<T> {
	Deploy {
		app,
		state: DeployState::CheckIfNeeded,
		#[cfg(feature = "deploy")]
		home_chain_id,
		#[cfg(feature = "deploy")]
		foreign_chain_id,
	}
}

pub struct Deploy<T: Transport + Clone> {
	app: Arc<App<T>>,
	#[cfg(feature = "deploy")]
	state: DeployState<T>,
	#[cfg(not(feature = "deploy"))]
	state: DeployState,
	#[cfg(feature = "deploy")]
	home_chain_id: u64,
	#[cfg(feature = "deploy")]
	foreign_chain_id: u64,
}

impl<T: Transport + Clone> Future for Deploy<T> {
	type Item = Deployed;
	type Error = Error;

	fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
		loop {
			let _next_state = match self.state {
				DeployState::CheckIfNeeded => match Database::load(&self.app.database_path).map_err(ErrorKind::from) {
					Ok(database) => return Ok(Deployed::Existing(database).into()),
					Err(ErrorKind::MissingFile(_e)) => {
						#[cfg(feature = "deploy")] {
							println!("deploy");
							let main_data = self.app.home_bridge.constructor(
								self.app.config.home.contract.bin.clone().0,
								self.app.config.authorities.required_signatures,
								self.app.config.authorities.accounts.clone(),
								self.app.config.estimated_gas_cost_of_withdraw
							);
							let test_data = self.app.foreign_bridge.constructor(
								self.app.config.foreign.contract.bin.clone().0,
								self.app.config.authorities.required_signatures,
								self.app.config.authorities.accounts.clone(),
								self.app.config.estimated_gas_cost_of_withdraw
							);

							let main_tx = Transaction {
								nonce: U256::zero(),
								gas_price: self.app.config.txs.home_deploy.gas_price.into(),
								gas: self.app.config.txs.home_deploy.gas.into(),
								action: Action::Create,
								value: U256::zero(),
								data: main_data.into(),
							};

							let test_tx = Transaction {
								nonce: U256::zero(),
								gas_price: self.app.config.txs.foreign_deploy.gas_price.into(),
								gas: self.app.config.txs.foreign_deploy.gas.into(),
								action: Action::Create,
								value: U256::zero(),
								data: test_data.into(),
							};

							let main_future = api::send_transaction_with_nonce(self.app.connections.home.clone(), self.app.clone(),
																			   self.app.config.home.clone(), main_tx, self.home_chain_id,
																			   TransactionWithConfirmation(self.app.connections.home.clone(), self.app.config.home.poll_interval, self.app.config.home.required_confirmations));

							let test_future = api::send_transaction_with_nonce(self.app.connections.foreign.clone(), self.app.clone(),
																			   self.app.config.foreign.clone(), test_tx, self.foreign_chain_id,
																			   TransactionWithConfirmation(self.app.connections.foreign.clone(), self.app.config.foreign.poll_interval, self.app.config.foreign.required_confirmations));

							DeployState::Deploying(main_future.join(test_future))
						}
						#[cfg(not(feature = "deploy"))] {
							return Err(ErrorKind::MissingFile(_e).into())
						}
					},
					Err(err) => return Err(err.into()),
				},
				#[cfg(feature = "deploy")]
				DeployState::Deploying(ref mut future) => {
					let (main_receipt, test_receipt) = try_ready!(future.poll());
					let database = Database {
						home_contract_address: main_receipt.contract_address.expect("contract creation receipt must have an address; qed"),
						foreign_contract_address: test_receipt.contract_address.expect("contract creation receipt must have an address; qed"),
						home_deploy: Some(main_receipt.block_number.low_u64()),
						foreign_deploy: Some(test_receipt.block_number.low_u64()),
						checked_deposit_relay: main_receipt.block_number.low_u64(),
						checked_withdraw_relay: test_receipt.block_number.low_u64(),
						checked_withdraw_confirm: test_receipt.block_number.low_u64(),
					};
					return Ok(Deployed::New(database).into())
				},
			};
			#[allow(unreachable_code)] {
				self.state = _next_state;
			}
		}
	}
}
