use std::sync::{Arc, RwLock};
use std::ops;
use futures::{self, Future, Stream, stream::{Collect, IterOk, iter_ok, Buffered}, Poll};
use web3::Transport;
use web3::types::{U256, H520, Address, Bytes, FilterBuilder};
use api::{self, LogStream};
use app::App;
use contracts::foreign;
use util::web3_filter;
use database::Database;
use error::{Error, ErrorKind};
use message_to_mainnet::{MessageToMainnet, MESSAGE_LENGTH};
use ethcore_transaction::{Transaction, Action};
use itertools::Itertools;
use super::nonce::{NonceCheck, SendRawTransaction};

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
		future: Collect<Buffered<IterOk<::std::vec::IntoIter<NonceCheck<T, SendRawTransaction<T>>>, Error>>>,
		block: u64,
	},
	/// All withdraws till given block has been confirmed.
	Yield(Option<u64>),
}

pub fn create_withdraw_confirm<T: Transport + Clone>(app: Arc<App<T>>, init: &Database, foreign_balance: Arc<RwLock<Option<U256>>>, foreign_chain_id: u64, foreign_gas_price: Arc<RwLock<u64>>) -> WithdrawConfirm<T> {
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
		foreign_chain_id,
		foreign_gas_price,
	}
}

pub struct WithdrawConfirm<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: WithdrawConfirmState<T>,
	foreign_contract: Address,
	foreign_balance: Arc<RwLock<Option<U256>>>,
	foreign_chain_id: u64,
	foreign_gas_price: Arc<RwLock<u64>>,
}

impl<T: Transport> Stream for WithdrawConfirm<T> {
	type Item = u64;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		// borrow checker...
		let app = &self.app;
		let gas = self.app.config.txs.withdraw_confirm.gas.into();
		let gas_price = U256::from(*self.foreign_gas_price.read().unwrap());
		let contract = self.foreign_contract.clone();
		loop {
			let next_state = match self.state {
				WithdrawConfirmState::Wait => {
					let foreign_balance = self.foreign_balance.read().unwrap();
					if foreign_balance.is_none() {
						warn!("foreign contract balance is unknown");
						return Ok(futures::Async::NotReady);
					}

					let item = try_stream!(self.logs.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "polling foreign for withdrawals")));
					let len = item.logs.len();
					info!("got {} new withdraws to sign", len);
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

					let balance_required = gas * gas_price * U256::from(signatures.len());
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
								gas,
								gas_price,
								value: U256::zero(),
								data: payload.0,
								nonce: U256::zero(),
								action: Action::Call(contract),
							};
							api::send_transaction_with_nonce(self.app.connections.foreign.clone(), self.app.clone(), self.app.config.foreign.clone(),
															 tx, self.foreign_chain_id, SendRawTransaction(self.app.connections.foreign.clone()))
						}).collect_vec();

					info!("submitting {} signatures", len);
					WithdrawConfirmState::ConfirmWithdraws {
						future: iter_ok(confirmations).buffered(self.app.config.txs.withdraw_confirm.concurrency).collect(),
						block,
					}
				},
				WithdrawConfirmState::ConfirmWithdraws { ref mut future, block } => {
					let _ = try_ready!(future.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "sending signature submissions to foreign")));
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
