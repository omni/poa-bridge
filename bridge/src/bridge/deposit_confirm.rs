use std::ops::RangeFull;
use std::sync::{Arc, RwLock};

use ethcore_transaction::{Action, Transaction};
use futures::{Async, Future, Poll, Stream};
use futures::stream::{Collect, FuturesUnordered, futures_unordered};
use itertools::Itertools;
use web3::Transport;
use web3::types::{Address, Bytes, FilterBuilder, H520, U256};

use api::{eth_data_hash, LogStream, LogStreamInit, LogStreamItem, log_stream};
use app::App;
use contracts::home::HomeBridge;
use database::Database;
use error::{Error, ErrorKind};
use message_to_mainnet::MessageToMainnet;
use super::BridgeChecked;
use super::nonce::{NonceCheck, SendRawTransaction, send_transaction_with_nonce};
use util::web3_filter;

// A future representing all currently open calls to the Home
// contract's `submitSignature()` function.
type SubmitSignaturesFuture<T: Transport> =
	Collect<FuturesUnordered<NonceCheck<T, SendRawTransaction<T>>>>;

fn create_deposit_filter(contract: &HomeBridge, contract_address: Address) -> FilterBuilder {
	let filter = contract.events().deposit().create_filter();
	web3_filter(filter, contract_address)
}

fn create_submit_signature_payload(
	home_contract: &HomeBridge,
	deposit_message: Vec<u8>,
	signature: H520
) -> Bytes
{
	home_contract.functions().submit_signature()
		.input(signature.0.to_vec(), deposit_message)
		.into()
}

// Represents each possible state for the `DepositConfirm`.
enum State<T: Transport> {
	// Monitoring the Foreign chain for new `Deposit` events.  
	Initial,
	// Waiting for all calls to the Home contract's `submitSignature()`
	// function to finish.
	WaitingOnSubmitSignatures {
		future: SubmitSignaturesFuture<T>,
		last_block_checked: u64,
	},
	// All calls to the Home Contract's `submitSignature()` function
	// have finished. Yields the block number for the last block
	// checked for `Deposit` events on the Foreign chain. 
	Yield(Option<u64>),
}

pub struct DepositConfirm<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: State<T>,
	home_contract_address: Address,
	home_balance: Arc<RwLock<Option<U256>>>,
	home_chain_id: u64,
	home_gas_price: Arc<RwLock<u64>>,
}

pub fn create_deposit_confirm<T: Transport + Clone>(
	app: Arc<App<T>>,
	init: &Database,
	home_balance: Arc<RwLock<Option<U256>>>,
	home_chain_id: u64,
	home_gas_price: Arc<RwLock<u64>>
) -> DepositConfirm<T>
{
	let deposit_log_filter = create_deposit_filter(
		&app.home_bridge,
		init.home_contract_address
	);

	let logs_init = LogStreamInit {
		after: init.checked_deposit_confirm,
		request_timeout: app.config.home.request_timeout,
		poll_interval: app.config.home.poll_interval,
		confirmations: app.config.home.required_confirmations,
		filter: deposit_log_filter,
	};

	let deposit_log_stream = log_stream(
		app.connections.home.clone(),
		app.timer.clone(),
		logs_init
	);

	DepositConfirm {
		logs: deposit_log_stream,
		home_contract_address: init.home_contract_address,
		state: State::Initial,
		app,
		home_balance,
		home_chain_id,
		home_gas_price,
	}
} 

impl<T: Transport> Stream for DepositConfirm<T> {
	type Item = BridgeChecked;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		let app = &self.app;
		let home_config = &app.config.home;
		let home_conn = &app.connections.home;
		let home_contract = &app.home_bridge;
		let home_contract_address = self.home_contract_address;
		let home_chain_id = self.home_chain_id;
		let my_home_address = app.config.home.account;
		let gas = app.config.txs.deposit_confirm.gas.into();
		let gas_price = U256::from(*self.home_gas_price.read().unwrap());

		loop {
			let next_state = match self.state {
				State::Initial => {
					let home_balance = self.home_balance.read().unwrap();
					if home_balance.is_none() {
						warn!("home contract balance is unknown");
						return Ok(Async::NotReady);
					}
 
					let LogStreamItem { to: last_block_checked, logs, .. } =
						try_stream!(
							self.logs.poll().map_err(|e| {
								let context = "polling Home contract for Depoist event logs";
								ErrorKind::ContextualizedError(Box::new(e), context)
							})
						);

					let n_new_deposits = logs.len();
					info!("got {} new deposits to sign", n_new_deposits);

					let mut messages = logs.into_iter()
						.map(|log| {
							info!(
								"deposit is ready for signature submission. tx hash {}",
								log.transaction_hash.unwrap()
							);
							
							MessageToMainnet::from_deposit_log(log)
								.map(|msg| msg.to_bytes())
						})
						.collect::<Result<Vec<Vec<u8>>, Error>>()?;

					let signatures = messages.iter()
						.map(|message| {
							let signed_message = eth_data_hash(message.clone());
							app.keystore.sign(my_home_address, None, signed_message)
						})
						.map_results(|sig| H520::from(sig.into_electrum()))
						.fold_results(vec![], |mut acc, sig| {
							acc.push(sig);
							acc
						})
						.map_err(ErrorKind::SignError)?;

					let balance_required = gas * gas_price * U256::from(signatures.len());
					if balance_required > *home_balance.as_ref().unwrap() {
						return Err(ErrorKind::InsufficientFunds.into());
					}

					let submit_signature_calls = messages.drain(RangeFull)
						.zip(signatures.into_iter())
						.map(|(message, signature)| create_submit_signature_payload(
							home_contract,
							message,
							signature
						))
						.map(|payload| {
							let tx = Transaction {
								gas,
								gas_price,
								value: U256::zero(),
								data: payload.0,
								nonce: U256::zero(),
								action: Action::Call(home_contract_address),
							};

							send_transaction_with_nonce(
								home_conn.clone(),
								app.clone(),
								home_config.clone(),
								tx,
								home_chain_id,
								SendRawTransaction(home_conn.clone())
							)
						})
						.collect_vec();

					State::WaitingOnSubmitSignatures {
						future: futures_unordered(submit_signature_calls).collect(),
						last_block_checked,
					}
				},
				State::WaitingOnSubmitSignatures { ref mut future, last_block_checked } => {
					let _ = try_ready!(
						future.poll().map_err(|e| {
							let context = "sending signature submissions to home";
							ErrorKind::ContextualizedError(Box::new(e), context)
						})
					);
					info!("submitting signatures to home complete");
					State::Yield(Some(last_block_checked))
				},
				State::Yield(ref mut block) => match block.take() {
					Some(block) => {
						let checked = BridgeChecked::DepositConfirm(block);
						return Ok(Async::Ready(Some(checked)));
					},
					None => State::Initial,
				},
			};

			self.state = next_state;
		}
	}
}

