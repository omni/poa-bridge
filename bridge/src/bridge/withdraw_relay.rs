use std::sync::{Arc, RwLock};

use ethabi::RawLog;
use ethcore_transaction::{Action, Transaction};
use futures::{Async, Future, Stream, Poll};
use futures::stream::{Collect, FuturesUnordered, futures_unordered};
use itertools::Itertools;
use web3::Transport;
use web3::types::{Address, FilterBuilder, Log, U256};

use api::{log_stream, LogStream, LogStreamInit, LogStreamItem};
use app::App;
use contracts::{foreign::ForeignBridge, home::HomeBridge};
use database::Database;
use error::{Error, ErrorKind};
use super::BridgeChecked;
use super::nonce::{NonceCheck, SendRawTransaction, send_transaction_with_nonce};
use util::web3_filter;

// A future representing all currently open calls to the Home
// contract's `withdraw()` function.
type WithdrawsFuture<T: Transport> =
	Collect<FuturesUnordered<NonceCheck<T, SendRawTransaction<T>>>>;

fn create_withdraw_filter(foreign: &ForeignBridge, address: Address) -> FilterBuilder {
	let filter = foreign.events().withdraw().create_filter();
	web3_filter(filter, address)
}

fn create_withdraw_payload(
	foreign: &ForeignBridge,
	home: &HomeBridge,
	withdraw_event_log: Log
) -> Vec<u8>
{
	let raw_log = RawLog {
		topics: withdraw_event_log.topics,
		data: withdraw_event_log.data.0
	};

	let parsed = foreign.events().withdraw()
		.parse_log(raw_log)
		.unwrap();

	let tx_hash = withdraw_event_log.transaction_hash
		.expect("Withdraw event does not contain a `transaction_hash`")
		.0;

	home.functions().withdraw()
		.input(parsed.recipient, parsed.value, tx_hash)
}

// Represents each possible state for the `WithdrawRelay`.
enum State<T: Transport> {
	// Monitoring the Foreign chain for new `Withdraw` events.
	Initial,
	// Waiting for all calls to the Home Contract's `withdraw()`
	// function to finish. 
	WaitingOnWithdraws {
		future: WithdrawsFuture<T>,
		last_block_checked: u64,
	},
	// All calls to the Home Contract's `withdraw()` function have
	// finished. Yields the block number for the last block checked
	// for `Withdraw` events on the Foreign chain.
	Yield(Option<u64>),
}

pub struct WithdrawRelay<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: State<T>,
	home_contract_address: Address,
	home_balance: Arc<RwLock<Option<U256>>>,
	home_chain_id: u64,
	home_gas_price: Arc<RwLock<u64>>,
}

pub fn create_withdraw_relay<T: Transport>(
	app: Arc<App<T>>,
	init: &Database,
	home_balance: Arc<RwLock<Option<U256>>>,
	home_chain_id: u64,
	home_gas_price: Arc<RwLock<u64>>,
) -> WithdrawRelay<T>
{
	let withdraw_event_filter = create_withdraw_filter(
		&app.foreign_bridge,
		init.foreign_contract_address
	);

	let logs_init = LogStreamInit {
		after: init.checked_withdraw_relay,
		request_timeout: app.config.foreign.request_timeout,
		poll_interval: app.config.foreign.poll_interval,
		confirmations: app.config.foreign.required_confirmations,
		filter: withdraw_event_filter,
	};

	let withdraw_log_stream = log_stream(
		app.connections.foreign.clone(),
		app.timer.clone(),
		logs_init
	);

	WithdrawRelay {
		app,
		logs: withdraw_log_stream,
		state: State::Initial,
		home_contract_address: init.home_contract_address,
		home_balance,
		home_chain_id,
		home_gas_price,
	} 
}

impl<T: Transport> Stream for WithdrawRelay<T> {
	type Item = BridgeChecked;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		let app = &self.app;
		let home_config = &app.config.home;
		let home_conn = &app.connections.home;
		let home_contract = &app.home_bridge;
		let foreign_contract = &app.foreign_bridge;
		let gas_per_withdraw = self.app.config.txs.withdraw_relay.gas.into();

		loop {
			let next_state = match self.state {
				State::Initial => {
					let home_balance = self.home_balance.read().unwrap();

					if home_balance.is_none() {
						warn!("home contract balance is unknown");
						return Ok(Async::NotReady);
					}
					
					let LogStreamItem { to: last_block_checked, logs, .. } = try_stream!(
						self.logs.poll().map_err(|e| {
							let context = "polling Foreign contract for Withdraw event logs";
							ErrorKind::ContextualizedError(Box::new(e), context)
						})
					);

					let n_withdraws: U256 = logs.len().into();
					info!("found {} new Withdraw events", n_withdraws); 

					let gas_price = U256::from(*self.home_gas_price.read().unwrap());
					let balance_required = n_withdraws * gas_per_withdraw * gas_price;

					if balance_required > *home_balance.as_ref().unwrap() {
						return Err(ErrorKind::InsufficientFunds.into());
					}

					let withdraws = logs.into_iter()
						.map(|log| {
							let payload = create_withdraw_payload(
								foreign_contract,
								home_contract,
								log
							);

							let tx = Transaction {
								gas: gas_per_withdraw,
								gas_price,
								value: U256::zero(),
								data: payload,
								nonce: U256::zero(),
								action: Action::Call(self.home_contract_address),
							};
							
							send_transaction_with_nonce(
								home_conn.clone(),
								app.clone(),
								home_config.clone(),
								tx,
								self.home_chain_id,
								SendRawTransaction(home_conn.clone()),
							)
						})
						.collect_vec();

					State::WaitingOnWithdraws {
						future: futures_unordered(withdraws).collect(),
						last_block_checked,
					}
				},
				State::WaitingOnWithdraws { ref mut future, last_block_checked } => {
					let _ = try_ready!(
						future.poll().map_err(|e| {
							let context = "sending withdraws to home";
							ErrorKind::ContextualizedError(Box::new(e), context)
						})
					);
					info!("finished relaying withdraws to Home");
					State::Yield(Some(last_block_checked))
				},
				State::Yield(ref mut block) => match block.take() {
					Some(block) => {
						let checked = BridgeChecked::WithdrawRelay(block);
						return Ok(Async::Ready(Some(checked)));
					},
					None => State::Initial,
				},
			};
		
			self.state = next_state;
		}
	}
}
