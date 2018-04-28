use std::sync::{Arc, RwLock};
use std::ops;
use futures::{self, Future, Stream, Poll};
use futures::future::{JoinAll, join_all};
use tokio_timer::Timeout;
use web3::Transport;
use web3::types::{H256, U256, H520, Address, Bytes, FilterBuilder};
use api::{self, LogStream, ApiCall};
use app::App;
use contracts::foreign;
use util::web3_filter;
use database::Database;
use error::{Error, ErrorKind};
use message_to_mainnet::{MessageToMainnet, MESSAGE_LENGTH};
use transaction::prepare_raw_transaction;
use ethcore_transaction::{Transaction, Action};
use itertools::Itertools;

fn withdraws_filter(foreign: &foreign::ForeignBridge, address: Address) -> FilterBuilder {
	let filter = foreign.events().withdraw().create_filter();
	web3_filter(filter, address)
}

fn withdraw_submit_signature_payload(foreign: &foreign::ForeignBridge, withdraw_message: Vec<u8>, signature: H520) -> Bytes {
	assert_eq!(withdraw_message.len(), MESSAGE_LENGTH, "ForeignBridge never accepts messages with len != {} bytes; qed", MESSAGE_LENGTH);
	foreign.functions().submit_signature().input(signature.0.to_vec(), withdraw_message).into()
}

/// State of withdraw confirmation.
enum WithdrawConfirmState<T: Transport> {
	/// Withdraw confirm is waiting for logs.
	Wait,
	/// Confirming withdraws.
	ConfirmWithdraws {
		future: JoinAll<Vec<Timeout<ApiCall<H256, T::Out>>>>,
		block: u64,
	},
	/// All withdraws till given block has been confirmed.
	Yield(Option<u64>),
}

pub fn create_withdraw_confirm<T: Transport + Clone>(app: Arc<App<T>>, init: &Database, foreign_balance: Arc<RwLock<Option<U256>>>, foreign_chain_id: u64, foreign_nonce: Arc<RwLock<Option<U256>>>) -> WithdrawConfirm<T> {
	let logs_init = api::LogStreamInit {
		after: init.checked_withdraw_confirm,
		request_timeout: app.config.foreign.request_timeout,
		poll_interval: app.config.foreign.poll_interval,
		confirmations: app.config.foreign.required_confirmations,
		filter: withdraws_filter(&app.foreign_bridge, init.foreign_contract_address.clone()),
	};

	WithdrawConfirm {
		logs: api::log_stream(app.connections.foreign.clone(), app.timer.clone(), logs_init),
		foreign_contract: init.foreign_contract_address,
		state: WithdrawConfirmState::Wait,
		app,
		foreign_balance,
		foreign_nonce,
		foreign_chain_id,
	}
}

pub struct WithdrawConfirm<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: WithdrawConfirmState<T>,
	foreign_contract: Address,
	foreign_balance: Arc<RwLock<Option<U256>>>,
	foreign_nonce: Arc<RwLock<Option<U256>>>,
	foreign_chain_id: u64,
}

impl<T: Transport> Stream for WithdrawConfirm<T> {
	type Item = u64;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		// borrow checker...
		let app = &self.app;
		let gas = self.app.config.txs.withdraw_confirm.gas.into();
		let gas_price = self.app.config.txs.withdraw_confirm.gas_price.into();
		let contract = self.foreign_contract.clone();
		let foreign = &self.app.config.foreign;
		let chain_id = self.foreign_chain_id;
		loop {
			let next_state = match self.state {
				WithdrawConfirmState::Wait => {
					let foreign_balance = self.foreign_balance.read().unwrap();
					if foreign_balance.is_none() {
						warn!("foreign contract balance is unknown");
						return Ok(futures::Async::NotReady);
					}
					let foreign_nonce = self.foreign_nonce.read().unwrap();
					if foreign_nonce.is_none() {
						warn!("foreign nonce is unknown");
						return Ok(futures::Async::NotReady);
					}

					let item = try_stream!(self.logs.poll());
					info!("got {} new withdraws to sign", item.logs.len());
					let mut messages = item.logs
						.into_iter()
						.map(|log| {
							 info!("withdraw is ready for signature submission. tx hash {}", log.transaction_hash.unwrap());
							 Ok(MessageToMainnet::from_log(log)?.to_bytes())
						})
						.collect::<Result<Vec<_>, Error>>()?;

					info!("signing");

					let signatures = messages.clone()
						.into_iter()
						.map(|message|
							app.keystore.sign(self.app.config.foreign.account, None, api::eth_data_hash(message)))
						.map_results(|sig| H520::from(sig.into_electrum()))
						.fold_results(vec![], |mut acc, sig| {
							acc.push(sig);
							acc
						})
						.map_err(|e| ErrorKind::SignError(e))?;

					let block = item.to;

					let balance_required = U256::from(self.app.config.txs.withdraw_confirm.gas) * U256::from(self.app.config.txs.withdraw_confirm.gas_price) * U256::from(signatures.len());
					if balance_required > *foreign_balance.as_ref().unwrap() {
						return Err(ErrorKind::InsufficientFunds.into())
					}

					info!("signing complete");
					let confirmations = messages
						.drain(ops::RangeFull)
						.zip(signatures.into_iter())
						.map(|(withdraw_message, signature)| {
							 withdraw_submit_signature_payload(&app.foreign_bridge, withdraw_message, signature)
						})
						.map(|payload| {
							let tx = Transaction {
								gas, gas_price,
								value: U256::zero(),
								data: payload.0,
								nonce: foreign_nonce.unwrap(),
								action: Action::Call(contract),
							};
							prepare_raw_transaction(tx, app, foreign, chain_id)
						})
						.map_results(|tx| {
							info!("submitting signature");
							app.timer.timeout(
								api::send_raw_transaction(&app.connections.foreign, tx),
								app.config.foreign.request_timeout)
						})
						.fold_results(vec![], |mut acc, tx| {
							acc.push(tx);
							acc
						})?;

					info!("submitting {} signatures", confirmations.len());
					WithdrawConfirmState::ConfirmWithdraws {
						future: join_all(confirmations),
						block,
					}
				},
				WithdrawConfirmState::ConfirmWithdraws { ref mut future, block } => {
					let _ = try_ready!(future.poll());
					info!("submitting signatures complete");
					WithdrawConfirmState::Yield(Some(block))
				},
				WithdrawConfirmState::Yield(ref mut block) => match block.take() {
					None => {
						info!("waiting for new withdraws that should get signed");
						WithdrawConfirmState::Wait
					},
					some => return Ok(some.into()),
				}
			};
			self.state = next_state;
		}
	}
}
