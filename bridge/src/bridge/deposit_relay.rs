use std::sync::Arc;
use futures::{Future, Stream, Poll, future};
use web3::Transport;
use web3::types::{TransactionRequest, H256, Address, Bytes, Log, FilterBuilder};
use ethabi::RawLog;
use api::{LogStream, self};
use error::{Error, Result};
use database::Database;
use contracts::{home, foreign};
use util::web3_filter;
use app::App;
use super::batch::batch;
use super::sequentially::sequentially;

fn deposits_filter(home: &home::HomeBridge, address: Address) -> FilterBuilder {
	let filter = home.events().deposit().create_filter();
	web3_filter(filter, address)
}

fn deposit_relay_payload(home: &home::HomeBridge, foreign: &foreign::ForeignBridge, log: Log) -> Result<Bytes> {
	let raw_log = RawLog {
		topics: log.topics,
		data: log.data.0,
	};
	let deposit_log = home.events().deposit().parse_log(raw_log)?;
	let hash = log.transaction_hash.expect("log to be mined and contain `transaction_hash`");
	let payload = foreign.functions().deposit().input(deposit_log.recipient, deposit_log.value, hash.0);
	Ok(payload.into())
}

/// State of deposits relay.
enum DepositRelayState {
	/// Deposit relay is waiting for logs.
	Wait,
	/// Relaying deposits in progress.
	RelayDeposits {
		future: Box<Future<Item = Vec<H256>, Error = Error>>,
		block: u64,
	},
	/// All deposits till given block has been relayed.
	Yield(Option<u64>),
}

pub fn create_deposit_relay<T: Transport + Clone>(app: Arc<App<T>>, init: &Database) -> DepositRelay<T> {
	let logs_init = api::LogStreamInit {
		after: init.checked_deposit_relay,
		request_timeout: app.config.home.request_timeout,
		poll_interval: app.config.home.poll_interval,
		confirmations: app.config.home.required_confirmations,
		filter: deposits_filter(&app.home_bridge, init.home_contract_address),
	};
	DepositRelay {
		logs: api::log_stream(app.connections.home.clone(), app.timer.clone(), logs_init),
		foreign_contract: init.foreign_contract_address,
		state: DepositRelayState::Wait,
		app,
	}
}

pub struct DepositRelay<T: Transport> {
	app: Arc<App<T>>,
	logs: LogStream<T>,
	state: DepositRelayState,
	foreign_contract: Address,
}

impl<T: Transport> Stream for DepositRelay<T> where T::Out: 'static {
	type Item = u64;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				DepositRelayState::Wait => {
					let item = try_stream!(self.logs.poll());
					let len = item.logs.len();
					info!("got {} new deposits to relay", len);

					let mut deposits0 = item.logs
						.into_iter()
						.map(|log| deposit_relay_payload(&self.app.home_bridge, &self.app.foreign_bridge, log))
						.collect::<Result<Vec<_>>>()?
						.into_iter()
						.map(|payload| TransactionRequest {
							from: self.app.config.foreign.account,
							to: Some(self.foreign_contract.clone()),
							gas: Some(self.app.config.txs.deposit_relay.gas.into()),
							gas_price: Some(self.app.config.txs.deposit_relay.gas_price.into()),
							value: None,
							data: Some(payload),
							nonce: None,
							condition: None,
						})
						.map(|request| {
							self.app.timer.timeout(
								api::send_transaction(&self.app.connections.foreign, request),
								self.app.config.foreign.request_timeout)
						});


					let mut batches = batch(deposits0,|items| Box::new(future::join_all(items)), self.app.config.foreign.simultaneous_requests_per_batch);

					info!("relaying {} deposits", len);
					DepositRelayState::RelayDeposits {
						future: sequentially(batches),
						block: item.to,
					}
				},
				DepositRelayState::RelayDeposits { ref mut future, block } => {
					let _ = try_ready!(future.poll());
					info!("deposit relay completed");
					DepositRelayState::Yield(Some(block))
				},
				DepositRelayState::Yield(ref mut block) => match block.take() {
					None => DepositRelayState::Wait,
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
	use web3::types::{Log, Bytes};
	use contracts::{home, foreign};
	use super::deposit_relay_payload;

	#[test]
	fn test_deposit_relay_payload() {
		let home = home::HomeBridge::default();
		let foreign = foreign::ForeignBridge::default();

		let data = "000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0".from_hex().unwrap();
		let log = Log {
			data: data.into(),
			topics: vec!["e1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c".into()],
			transaction_hash: Some("884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364".into()),
			..Default::default()
		};

		let payload = deposit_relay_payload(&home, &foreign, log).unwrap();
		let expected: Bytes = "26b3293f000000000000000000000000aff3454fce5edbc8cca8697c15331677e6ebcccc00000000000000000000000000000000000000000000000000000000000000f0884edad9ce6fa2440d8a54cc123490eb96d2768479d49ff9c7366125a9424364".from_hex().unwrap().into();
		assert_eq!(expected, payload);
	}
}
