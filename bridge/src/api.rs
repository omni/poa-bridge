use std::time::Duration;
use serde::de::DeserializeOwned;
use serde_json::Value;
use futures::{Future, Stream, Poll};
use tokio_timer::{Timer, Interval, Timeout};
use web3::{self, api, Transport};
use web3::api::Namespace;
use web3::types::{Log, Filter, H256, U256, FilterBuilder, Bytes, Address, CallRequest, BlockNumber};
use web3::helpers::{self, CallResult};
use error::{Error, ErrorKind};

/// Imperative alias for web3 function.
pub use web3::confirm::send_raw_transaction_with_confirmation;

/// Wrapper type for `CallResult`
pub struct ApiCall<T, F> {
	future: CallResult<T, F>,
	message: &'static str,
}

impl<T: DeserializeOwned, F: Future<Item = Value, Error = web3::Error>>Future for ApiCall<T, F> {
	type Item = T;
	type Error = Error;

	fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
		trace!(target: "bridge", "{}", self.message);
		self.future.poll().map_err(ErrorKind::Web3).map_err(Into::into)
	}
}

/// Imperative wrapper for web3 function.
pub fn net_version<T: Transport>(transport: T) -> ApiCall<String, T::Out> {
	ApiCall {
		future: CallResult::new(transport.execute("net_version", vec![])),
		message: "net_version",
	}
}

/// Imperative wrapper for web3 function.
pub fn eth_get_transaction_count<T: Transport>(transport: T, address: Address, block: Option<BlockNumber>) -> ApiCall<U256, T::Out> {
	// we are not using Eth.balance() because it converts None block into `latest`
	// while we want `pending` because there might have not been enough time since
	// the last transaction to get it mined.
	let address = helpers::serialize(&address);
	let block = helpers::serialize(&block.unwrap_or(BlockNumber::Pending));
	ApiCall {
		future: CallResult::new(transport.execute("eth_getTransactionCount", vec![address, block])),
		message: "net_version",
	}
}


use serde_json;
/// trimming the null from the tail because at least some RPC servers require a topic to be present
/// if there's a null
/// FIXME: this is not a great fix long term
fn trim_filter(filter: &Filter) -> serde_json::Value {
	fn trim_filter1(vals: &mut Vec<serde_json::Value>) {
		loop {
			match vals.pop() {
				None => {
					return;
				},
				Some(serde_json::Value::Null) => (),
				Some(v) => {
					vals.push(v);
					return;
				}
			}
		}
	}
	match helpers::serialize(filter) {
		serde_json::Value::Object(mut map) => {
			for (k, v) in map.iter_mut() {
				if k == "topics" {
					match v {
						&mut serde_json::Value::Array(ref mut v) => trim_filter1(v),
						_ => (),
					}
				}
			}
			serde_json::Value::Object(map)
		}
		val => val,
	}
}

/// Imperative wrapper for web3 function.
pub fn logs<T: Transport>(transport: T, filter: &Filter) -> ApiCall<Vec<Log>, T::Out> {
	let filter = trim_filter(filter);
	ApiCall {
		future: CallResult::new(transport.execute("eth_getLogs", vec![filter])),
		message: "eth_getLogs",
	}
}

/// Imperative wrapper for web3 function.
pub fn block_number<T: Transport>(transport: T) -> ApiCall<U256, T::Out> {
	ApiCall {
		future: api::Eth::new(transport).block_number(),
		message: "eth_blockNumber",
	}
}

/// Imperative wrapper for web3 function.
pub fn balance<T: Transport>(transport: T, address: Address, block: Option<BlockNumber>) -> ApiCall<U256, T::Out> {
	// we are not using Eth.balance() because it converts None block into `latest`
	// while we want `pending` because there might have not been enough time since
	// the last transaction to get it mined.
	let address = helpers::serialize(&address);
	let block = helpers::serialize(&block.unwrap_or(BlockNumber::Pending));
	ApiCall {
		future: CallResult::new(transport.execute("eth_getBalance", vec![address, block])),
		message: "eth_getBalance",
	}
}

/// Imperative wrapper for web3 function.
pub fn send_raw_transaction<T: Transport>(transport: T, tx: Bytes) -> ApiCall<H256, T::Out> {
	ApiCall {
		future: api::Eth::new(transport).send_raw_transaction(tx),
		message: "eth_sendRawTransaction",
	}
}

pub use bridge::nonce::send_transaction_with_nonce;

/// Imperative wrapper for web3 function.
pub fn call<T: Transport>(transport: T, address: Address, payload: Bytes) -> ApiCall<Bytes, T::Out> {
	let future = api::Eth::new(transport).call(CallRequest {
		from: None,
		to: address,
		gas: None,
		gas_price: None,
		value: None,
		data: Some(payload),
	}, None);

	ApiCall {
		future,
		message: "eth_call",
	}
}

/// Returns a eth_sign-compatible hash of data to sign.
/// The data is prepended with special message to prevent
/// chosen-plaintext attacks.
pub fn eth_data_hash(mut data: Vec<u8>) -> H256 {
	use keccak_hash::keccak;
	let mut message_data =
		format!("\x19Ethereum Signed Message:\n{}", data.len())
			.into_bytes();
	message_data.append(&mut data);
	keccak(message_data)
}

/// Used for `LogStream` initialization.
pub struct LogStreamInit {
	pub after: u64,
	pub filter: FilterBuilder,
	pub request_timeout: Duration,
	pub poll_interval: Duration,
	pub confirmations: usize,
}

/// Contains all logs matching `LogStream` filter in inclusive range `[from, to]`.
#[derive(Debug, PartialEq)]
pub struct LogStreamItem {
	pub from: u64,
	pub to: u64,
	pub logs: Vec<Log>,
}

/// Log Stream state.
enum LogStreamState<T: Transport> {
	/// Log Stream is waiting for timer to poll.
	Wait,
	/// Fetching best block number.
	FetchBlockNumber(Timeout<ApiCall<U256, T::Out>>),
	/// Fetching logs for new best block.
	FetchLogs {
		from: u64,
		to: u64,
		future: Timeout<ApiCall<Vec<Log>, T::Out>>,
	},
	/// All logs has been fetched.
	NextItem(Option<LogStreamItem>),
}

/// Creates new `LogStream`.
pub fn log_stream<T: Transport>(transport: T, timer: Timer, init: LogStreamInit) -> LogStream<T> {
	LogStream {
		transport,
		interval: timer.interval(init.poll_interval),
		timer,
		state: LogStreamState::Wait,
		after: init.after,
		filter: init.filter,
		confirmations: init.confirmations,
		request_timeout: init.request_timeout,
	}
}

/// Stream of confirmed logs.
pub struct LogStream<T: Transport> {
	transport: T,
	timer: Timer,
	interval: Interval,
	state: LogStreamState<T>,
	after: u64,
	filter: FilterBuilder,
	confirmations: usize,
	request_timeout: Duration,
}

impl<T: Transport> Stream for LogStream<T> {
	type Item = LogStreamItem;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				LogStreamState::Wait => {
					let _ = try_stream!(self.interval.poll());
					LogStreamState::FetchBlockNumber(self.timer.timeout(block_number(&self.transport), self.request_timeout))
				},
				LogStreamState::FetchBlockNumber(ref mut future) => {
					let last_block = try_ready!(future.poll()).low_u64();
					let last_confirmed_block = last_block.saturating_sub(self.confirmations as u64);
					if last_confirmed_block > self.after {
						let from = self.after + 1;
						let filter = self.filter.clone()
							.from_block(from.into())
							.to_block(last_confirmed_block.into())
							.build();
						LogStreamState::FetchLogs {
							from: from,
							to: last_confirmed_block,
							future: self.timer.timeout(logs(&self.transport, &filter), self.request_timeout),
						}
					} else {
						LogStreamState::Wait
					}
				},
				LogStreamState::FetchLogs { ref mut future, from, to } => {
					let logs = try_ready!(future.poll());
					let item = LogStreamItem {
						from,
						to,
						logs,
					};

					self.after = to;
					LogStreamState::NextItem(Some(item))
				},
				LogStreamState::NextItem(ref mut item) => match item.take() {
					None => LogStreamState::Wait,
					some => return Ok(some.into()),
				},
			};

			self.state = next_state;
		}
	}
}
