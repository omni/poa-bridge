use std::sync::Arc;
use futures::{Future, Poll, future};
use web3::Transport;
use web3::confirm::SendTransactionWithConfirmation;
use web3::types::U256;
use app::App;
use database::Database;
use error::{Error, ErrorKind};
use api;
use transaction::prepare_raw_transaction;
use ethcore_transaction::{Transaction, Action};

pub enum Deployed {
	/// No existing database found. Deployed new contracts.
	New(Database),
	/// Reusing existing contracts.
	Existing(Database),
}

enum DeployState<T: Transport + Clone> {
	CheckIfNeeded,
	Deploying(future::Join<SendTransactionWithConfirmation<T>, SendTransactionWithConfirmation<T>>),
}

pub fn create_deploy<T: Transport + Clone>(app: Arc<App<T>>, home_chain_id: u64, foreign_chain_id: u64, home_nonce: U256, foreign_nonce: U256) -> Deploy<T> {
	Deploy {
		app,
		state: DeployState::CheckIfNeeded,
		home_chain_id,
		foreign_chain_id,
		home_nonce,
		foreign_nonce,
	}
}

pub struct Deploy<T: Transport + Clone> {
	app: Arc<App<T>>,
	state: DeployState<T>,
	home_chain_id: u64,
	foreign_chain_id: u64,
	home_nonce: U256,
	foreign_nonce: U256,
}

impl<T: Transport + Clone> Future for Deploy<T> {
	type Item = Deployed;
	type Error = Error;

	fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
		loop {
			let next_state = match self.state {
				DeployState::CheckIfNeeded => match Database::load(&self.app.database_path).map_err(ErrorKind::from) {
					Ok(database) => return Ok(Deployed::Existing(database).into()),
					Err(ErrorKind::MissingFile(_)) => {
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
							nonce: self.home_nonce,
							gas_price: self.app.config.txs.home_deploy.gas_price.into(),
							gas: self.app.config.txs.home_deploy.gas.into(),
							action: Action::Create,
							value: U256::zero(),
							data: main_data.into(),
						};

						let test_tx = Transaction {
							nonce: self.foreign_nonce,
							gas_price: self.app.config.txs.foreign_deploy.gas_price.into(),
							gas: self.app.config.txs.foreign_deploy.gas.into(),
							action: Action::Create,
							value: U256::zero(),
							data: test_data.into(),
						};

						let main_future = api::send_raw_transaction_with_confirmation(
							self.app.connections.home.clone(),
							prepare_raw_transaction(main_tx, &self.app, &self.app.config.home, self.home_chain_id)?,
							self.app.config.home.poll_interval,
							self.app.config.home.required_confirmations
						);

						let test_future = api::send_raw_transaction_with_confirmation(
							self.app.connections.foreign.clone(),
							prepare_raw_transaction(test_tx, &self.app, &self.app.config.foreign, self.foreign_chain_id)?,
							self.app.config.foreign.poll_interval,
							self.app.config.foreign.required_confirmations
						);

						DeployState::Deploying(main_future.join(test_future))
					},
					Err(err) => return Err(err.into()),
				},
				DeployState::Deploying(ref mut future) => {
					let (main_receipt, test_receipt) = try_ready!(future.poll().map_err(ErrorKind::Web3));
					let database = Database {
						home_contract_address: main_receipt.contract_address.expect("contract creation receipt must have an address; qed"),
						foreign_contract_address: test_receipt.contract_address.expect("contract creation receipt must have an address; qed"),
						home_deploy: main_receipt.block_number.low_u64(),
						foreign_deploy: test_receipt.block_number.low_u64(),
						checked_deposit_relay: main_receipt.block_number.low_u64(),
						checked_withdraw_relay: test_receipt.block_number.low_u64(),
						checked_withdraw_confirm: test_receipt.block_number.low_u64(),
					};
					return Ok(Deployed::New(database).into())
				},
			};

			self.state = next_state;
		}
	}
}
