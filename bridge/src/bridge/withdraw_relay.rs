use std::sync::{Arc, RwLock};
use futures::{self, Future, Stream, stream::{Collect, iter_ok, IterOk, Buffered}, Poll};
use futures::future::{JoinAll, join_all, Join};
use tokio_timer::Timeout;
use web3::Transport;
use web3::types::{U256, Address, FilterBuilder, Log, Bytes};
use ethabi::{RawLog, self};
use app::App;
use api::{self, LogStream, ApiCall};
use contracts::foreign;
use util::web3_filter;
use database::Database;
use error::{self, Error, ErrorKind};
use message_to_mainnet::MessageToMainnet;
use signature::Signature;
use ethcore_transaction::{Transaction, Action};
use super::nonce::{NonceCheck, SendRawTransaction};
use itertools::Itertools;

/// returns a filter for `ForeignBridge.CollectedSignatures` events
fn collected_signatures_filter(foreign: &foreign::ForeignBridge, address: Address) -> FilterBuilder {
	let filter = foreign.events().collected_signatures().create_filter();
	web3_filter(filter, address)
}

/// payloads for calls to `ForeignBridge.signature` and `ForeignBridge.message`
/// to retrieve the signatures (v, r, s) and messages
/// which the withdraw relay process should later relay to `HomeBridge`
/// by calling `HomeBridge.withdraw(v, r, s, message)`
#[derive(Debug, PartialEq)]
struct RelayAssignment {
	signature_payloads: Vec<Bytes>,
	message_payload: Bytes,
}

fn signatures_payload(foreign: &foreign::ForeignBridge, required_signatures: u32, my_address: Address, log: Log) -> error::Result<Option<RelayAssignment>> {
	// convert web3::Log to ethabi::RawLog since ethabi events can
	// only be parsed from the latter
	let raw_log = RawLog {
		topics: log.topics.into_iter().map(|t| t.0.into()).collect(),
		data: log.data.0,
	};
	let collected_signatures = foreign.events().collected_signatures().parse_log(raw_log)?;
	if collected_signatures.authority_responsible_for_relay != my_address.0.into() {
		info!("bridge not responsible for relaying transaction to home. tx hash: {}", log.transaction_hash.unwrap());
		// this authority is not responsible for relaying this transaction.
		// someone else will relay this transaction to home.
		return Ok(None);
	}
	let signature_payloads = (0..required_signatures).into_iter()
		.map(|index| foreign.functions().signature().input(collected_signatures.message_hash, index))
		.map(Into::into)
		.collect();
	let message_payload = foreign.functions().message().input(collected_signatures.message_hash).into();

	Ok(Some(RelayAssignment {
		signature_payloads,
		message_payload,
	}))
}

/// state of the withdraw relay state machine
pub enum WithdrawRelayState<T: Transport> {
	Wait,
	FetchMessagesSignatures {
		future: Join<
			JoinAll<Vec<Timeout<ApiCall<Bytes, T::Out>>>>,
			JoinAll<Vec<JoinAll<Vec<Timeout<ApiCall<Bytes, T::Out>>>>>>
		>,
		block: u64,
	},
	RelayWithdraws {
		future: Collect<Buffered<IterOk<::std::vec::IntoIter<NonceCheck<T, SendRawTransaction<T>>>, Error>>>,
		block: u64,
	},
	Yield(Option<u64>),
}

pub fn create_withdraw_relay<T: Transport + Clone>(app: Arc<App<T>>, init: &Database, home_balance: Arc<RwLock<Option<U256>>>, home_chain_id: u64, home_gas_price: Arc<RwLock<u64>>) -> WithdrawRelay<T> {
	let logs_init = api::LogStreamInit {
		after: init.checked_withdraw_relay,
		request_timeout: app.config.foreign.request_timeout,
		poll_interval: app.config.foreign.poll_interval,
		confirmations: app.config.foreign.required_confirmations,
		filter: collected_signatures_filter(&app.foreign_bridge, init.foreign_contract_address),
	};

	WithdrawRelay {
		logs: api::log_stream(app.connections.foreign.clone(), app.timer.clone(), logs_init),
		home_contract: init.home_contract_address,
		foreign_contract: init.foreign_contract_address,
		state: WithdrawRelayState::Wait,
		app,
		home_balance,
		home_chain_id,
		home_gas_price,
	}
}

pub struct WithdrawRelay<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: WithdrawRelayState<T>,
	foreign_contract: Address,
	home_contract: Address,
	home_balance: Arc<RwLock<Option<U256>>>,
	home_chain_id: u64,
	home_gas_price: Arc<RwLock<u64>>,
}

impl<T: Transport> Stream for WithdrawRelay<T> {
	type Item = u64;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		let app = &self.app;
		let gas = self.app.config.txs.withdraw_relay.gas.into();
		let gas_price = U256::from(*self.home_gas_price.read().unwrap());
		let contract = self.home_contract.clone();
		let home = &self.app.config.home;
		let t = &self.app.connections.home;
		let chain_id = self.home_chain_id;

		loop {
			let next_state = match self.state {
				WithdrawRelayState::Wait => {
					let item = try_stream!(self.logs.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "polling foreign for collected signatures")));
					info!("got {} new signed withdraws to relay", item.logs.len());
					let assignments = item.logs
						.into_iter()
						.map(|log| {
							 info!("collected signature is ready for relay: tx hash: {}", log.transaction_hash.unwrap());
							 signatures_payload(
								&self.app.foreign_bridge,
								self.app.config.authorities.required_signatures,
								self.app.config.foreign.account,
								log)
						})
						.collect::<error::Result<Vec<_>>>()?;

					let (signatures, messages): (Vec<_>, Vec<_>) = assignments.into_iter()
						.filter_map(|a| a)
						.map(|assignment| (assignment.signature_payloads, assignment.message_payload))
						.unzip();

					let message_calls = messages.into_iter()
						.map(|payload| {
							self.app.timer.timeout(
								api::call(&self.app.connections.foreign, self.foreign_contract.clone(), payload),
								self.app.config.foreign.request_timeout)
						})
						.collect::<Vec<_>>();

					let signature_calls = signatures.into_iter()
						.map(|payloads| {
							payloads.into_iter()
								.map(|payload| {
									self.app.timer.timeout(
										api::call(&self.app.connections.foreign, self.foreign_contract.clone(), payload),
										self.app.config.foreign.request_timeout)
								})
								.collect::<Vec<_>>()
						})
						.map(|calls| join_all(calls))
						.collect::<Vec<_>>();

					info!("fetching messages and signatures");
					WithdrawRelayState::FetchMessagesSignatures {
						future: join_all(message_calls).join(join_all(signature_calls)),
						block: item.to,
					}
				},
				WithdrawRelayState::FetchMessagesSignatures { ref mut future, block } => {
					let home_balance = self.home_balance.read().unwrap();
					if home_balance.is_none() {
						warn!("home contract balance is unknown");
						return Ok(futures::Async::NotReady);
					}

					let (messages_raw, signatures_raw) = try_ready!(future.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "fetching messages and signatures from foreign")));
					info!("fetching messages and signatures complete");
					assert_eq!(messages_raw.len(), signatures_raw.len());

					let balance_required = gas * gas_price * U256::from(messages_raw.len());
					if balance_required > *home_balance.as_ref().unwrap() {
						return Err(ErrorKind::InsufficientFunds.into())
					}

					let messages = messages_raw
						.iter()
						.map(|message| {
							app.foreign_bridge.functions().message().output(message.0.as_slice()).map(Bytes)
						})
						.collect::<ethabi::Result<Vec<_>>>()
						.map_err(error::Error::from)?;

					let len = messages.len();

					let signatures = signatures_raw
						.iter()
						.map(|signatures|
							signatures.iter().map(
								|signature| {
									Signature::from_bytes(
										app.foreign_bridge
											.functions()
											.signature()
											.output(signature.0.as_slice())?
											.as_slice())
								}
							)
							.collect::<Result<Vec<_>, Error>>()
							.map_err(error::Error::from)
						)
						.collect::<error::Result<Vec<_>>>()?;

					let relays = messages.into_iter()
						.zip(signatures.into_iter())
						.map(|(message, signatures)| {
							let payload: Bytes = app.home_bridge.functions().withdraw().input(
								signatures.iter().map(|x| x.v),
								signatures.iter().map(|x| x.r),
								signatures.iter().map(|x| x.s),
								message.clone().0).into();
							let gas_price = MessageToMainnet::from_bytes(message.0.as_slice()).mainnet_gas_price;
							let tx = Transaction {
									gas,
									gas_price,
									value: U256::zero(),
									data: payload.0,
									nonce: U256::zero(),
									action: Action::Call(contract),
								};
							    api::send_transaction_with_nonce(t.clone(), app.clone(), home.clone(), tx, chain_id, SendRawTransaction(t.clone()))
							}).collect_vec();

					info!("relaying {} withdraws", len);
					WithdrawRelayState::RelayWithdraws {
						future: iter_ok(relays).buffered(self.app.config.txs.withdraw_relay.concurrency).collect(),
						block,
					}
				},
				WithdrawRelayState::RelayWithdraws { ref mut future, block } => {
					let _ = try_ready!(future.poll().map_err(|e| ErrorKind::ContextualizedError(Box::new(e), "sending withdrawal to home")));
					info!("relaying withdraws complete");
					WithdrawRelayState::Yield(Some(block))
				},
				WithdrawRelayState::Yield(ref mut block) => match block.take() {
					None => {
						info!("waiting for signed withdraws to relay");
						WithdrawRelayState::Wait
					},
					some => return Ok(some.into()),
				}
			};
			self.state = next_state;
		}
	}
}

#[cfg(test)]
mod tests {
	use rustc_hex::FromHex;
	use web3::types::{Log, Bytes, Address};
	use contracts::foreign;
	use super::signatures_payload;

	#[test]
	fn test_signatures_payload() {
		let foreign = foreign::ForeignBridge::default();
		let my_address = "aff3454fce5edbc8cca8697c15331677e6ebcccc".into();

		let data = "000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0".from_hex().unwrap();

		let log = Log {
			data: data.into(),
			topics: vec!["eb043d149eedb81369bec43d4c3a3a53087debc88d2525f13bfaa3eecda28b5c".into()],
			transaction_hash: Some("884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364".into()),
			address: Address::zero(),
			block_hash: None,
			transaction_index: None,
			log_index: None,
			transaction_log_index: None,
			log_type: None,
			block_number: None,
			removed: None,
		};

		let assignment = signatures_payload(&foreign, 2, my_address, log).unwrap().unwrap();
		let expected_message: Bytes = "490a32c600000000000000000000000000000000000000000000000000000000000000f0".from_hex().unwrap().into();
		let expected_signatures: Vec<Bytes> = vec![
			"1812d99600000000000000000000000000000000000000000000000000000000000000f00000000000000000000000000000000000000000000000000000000000000000".from_hex().unwrap().into(),
			"1812d99600000000000000000000000000000000000000000000000000000000000000f00000000000000000000000000000000000000000000000000000000000000001".from_hex().unwrap().into(),
		];
		assert_eq!(expected_message, assignment.message_payload);
		assert_eq!(expected_signatures, assignment.signature_payloads);
	}

	#[test]
	fn test_signatures_payload_not_ours() {
		let foreign = foreign::ForeignBridge::default();
		let my_address = "aff3454fce5edbc8cca8697c15331677e6ebcccd".into();

		let data = "000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0".from_hex().unwrap();

		let log = Log {
			data: data.into(),
			topics: vec!["eb043d149eedb81369bec43d4c3a3a53087debc88d2525f13bfaa3eecda28b5c".into()],
			transaction_hash: Some("884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364".into()),
			address: Address::zero(),
			block_hash: None,
			transaction_index: None,
			log_index: None,
			transaction_log_index: None,
			log_type: None,
			block_number: None,
			removed: None,
		};

		let assignment = signatures_payload(&foreign, 2, my_address, log).unwrap();
		assert_eq!(None, assignment);
	}
}
