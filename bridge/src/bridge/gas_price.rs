use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::{Async, Future, Poll, Stream};
use hyper::{Chunk, client::{HttpConnector, Connect}, Client, Uri, Error as HyperError};
use hyper_tls::HttpsConnector;
use serde_json as json;
use tokio_core::reactor::Handle;
use tokio_timer::{Interval, Timer, Timeout};

use config::{GasPriceSpeed, Node};
use error::Error;

const CACHE_TIMEOUT_DURATION: Duration = Duration::from_secs(5 * 60);

enum State<F> {
    Initial,
    WaitingForResponse(Timeout<F>),
    Yield(Option<u64>),
}

pub trait Retriever {
	type Item: AsRef<[u8]>;
	type Future: Future<Item = Self::Item, Error = Error>;
	fn retrieve(&self, uri: &Uri) -> Self::Future;
}

impl<C, B> Retriever for Client<C, B> where C: Connect, B: Stream<Item = Chunk, Error = HyperError> + 'static {
	type Item = Chunk;
	type Future = Box<Future<Item = Self::Item, Error = Error>>;

	fn retrieve(&self, uri: &Uri) -> Self::Future {
		Box::new(
			self.get(uri.clone())
				.and_then(|resp| resp.body().concat2())
				.map_err(|e| e.into())
		)
	}
}

pub type StandardGasPriceStream = GasPriceStream<Box<Future<Item = Chunk, Error = Error>>, Client<HttpsConnector<HttpConnector>>, Chunk>;

pub struct GasPriceStream<F, R, I> where I: AsRef<[u8]>, F: Future<Item = I, Error = Error>, R: Retriever<Future = F, Item = F::Item> {
    state: State<F>,
    retriever: R,
    uri: Uri,
    speed: GasPriceSpeed,
    request_timer: Timer,
    interval: Interval,
    last_price: u64,
    request_timeout: Duration,
}

impl StandardGasPriceStream {
	pub fn new(node: &Node, handle: &Handle, timer: &Timer) -> Self {
		let client = Client::configure()
			.connector(HttpsConnector::new(4, handle).unwrap())
			.build(handle);
		GasPriceStream::new_with_retriever(node, client, timer)
	}
}

impl<F, R, I> GasPriceStream<F, R, I> where I: AsRef<[u8]>, F: Future<Item = I, Error = Error>, R: Retriever<Future = F, Item = F::Item> {
    pub fn new_with_retriever(node: &Node, retriever: R, timer: &Timer) -> Self {
        let uri: Uri = node.gas_price_oracle_url.clone().unwrap().parse().unwrap();

        GasPriceStream {
            state: State::Initial,
            retriever,
            uri,
            speed: node.gas_price_speed,
            request_timer: timer.clone(),
            interval: timer.interval_at(Instant::now(), CACHE_TIMEOUT_DURATION),
            last_price: node.default_gas_price,
            request_timeout: node.gas_price_timeout,
        }
    }
}

impl<F, R, I> Stream for GasPriceStream<F, R, I> where I: AsRef<[u8]>, F: Future<Item = I, Error = Error>, R: Retriever<Future = F, Item = F::Item> {
    type Item = u64;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let next_state = match self.state {
                State::Initial => {
                    let _ = try_stream!(self.interval.poll());

                    let request = self.retriever.retrieve(&self.uri);

                    let request_future = self.request_timer
                        .timeout(request, self.request_timeout);

                    State::WaitingForResponse(request_future)
                },
                State::WaitingForResponse(ref mut request_future) => {
                    match request_future.poll() {
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(chunk)) => {
                            match json::from_slice::<HashMap<String, json::Value>>(chunk.as_ref()) {
								Ok(json_obj) => {
									match json_obj.get(self.speed.as_str()) {
										Some(json::Value::Number(price)) => State::Yield(Some((price.as_f64().unwrap() * 1_000_000_000.0).trunc() as u64)),
										_ => {
											error!("Invalid or missing gas price ({}) in the gas price oracle response: {}", self.speed.as_str(), String::from_utf8_lossy(&*chunk.as_ref()));
											State::Yield(Some(self.last_price))
										},
									}
								},
								Err(e) => {
									error!("Error while parsing response from gas price oracle: {:?} {}", e, String::from_utf8_lossy(&*chunk.as_ref()));
									State::Yield(Some(self.last_price))
								}
							}
                        },
                        Err(e) => {
                            error!("Error while fetching gas price: {:?}", e);
							State::Yield(Some(self.last_price))
                        },
                    }
                },
                State::Yield(ref mut opt) => match opt.take() {
					None => State::Initial,
					Some(price) => {
						if price != self.last_price {
							info!("Gas price: {} gwei", (price as f64) / 1_000_000_000.0);
							self.last_price = price;
						}
						return Ok(Async::Ready(Some(price)))
                    },
				}
            };

            self.state = next_state;
        }
    }
}

#[cfg(test)]
mod tests {

	use super::*;
	use error::{Error, ErrorKind};
	use futures::{Async, future::{err, ok, FutureResult}};
	use config::{Node, NodeInfo};
	use tokio_timer::Timer;
	use std::time::Duration;
	use std::path::PathBuf;
	use web3::types::Address;
	use std::str::FromStr;

	struct ErroredRequest;

	impl Retriever for ErroredRequest {
		type Item = Vec<u8>;
		type Future = FutureResult<Self::Item, Error>;

		fn retrieve(&self, _uri: &Uri) -> <Self as Retriever>::Future {
			err(ErrorKind::OtherError("something went wrong".into()).into())
		}
	}

	#[test]
	fn errored_request() {
		let node = Node {
			account: Address::new(),
			request_timeout: Duration::from_secs(5),
			poll_interval: Duration::from_secs(1),
			required_confirmations: 0,
			rpc_host: "https://rpc".into(),
			rpc_port: 443,
			password: PathBuf::from("password"),
			info: NodeInfo::default(),
			gas_price_oracle_url: Some("https://gas.price".into()),
			gas_price_speed: GasPriceSpeed::from_str("fast").unwrap(),
			gas_price_timeout: Duration::from_secs(5),
			default_gas_price: 15_000_000_000,
		};
		let timer = Timer::default();
		let mut stream = GasPriceStream::new_with_retriever(&node, ErroredRequest, &timer);
		loop {
			match stream.poll() {
				Ok(Async::Ready(Some(v))) => {
					assert_eq!(v, node.default_gas_price);
					break;
				},
				Err(_) => panic!("should not error out"),
				_ => (),
			}
		}
	}


	struct BadJson;

	impl Retriever for BadJson {
		type Item = String;
		type Future = FutureResult<Self::Item, Error>;

		fn retrieve(&self, _uri: &Uri) -> <Self as Retriever>::Future {
			ok("bad json".into())
		}
	}

	#[test]
	fn bad_json() {
		let node = Node {
			account: Address::new(),
			request_timeout: Duration::from_secs(5),
			poll_interval: Duration::from_secs(1),
			required_confirmations: 0,
			rpc_host: "https://rpc".into(),
			rpc_port: 443,
			password: PathBuf::from("password"),
			info: NodeInfo::default(),
			gas_price_oracle_url: Some("https://gas.price".into()),
			gas_price_speed: GasPriceSpeed::from_str("fast").unwrap(),
			gas_price_timeout: Duration::from_secs(5),
			default_gas_price: 15_000_000_000,
		};
		let timer = Timer::default();
		let mut stream = GasPriceStream::new_with_retriever(&node, BadJson, &timer);
		loop {
			match stream.poll() {
				Ok(Async::Ready(Some(v))) => {
					assert_eq!(v, node.default_gas_price);
					break;
				},
				Err(_) => panic!("should not error out"),
				_ => (),
			}
		}
	}


	struct UnexpectedJson;

	impl Retriever for UnexpectedJson {
		type Item = String;
		type Future = FutureResult<Self::Item, Error>;

		fn retrieve(&self, _uri: &Uri) -> <Self as Retriever>::Future {
			ok(r#"{"cow": "moo"}"#.into())
		}
	}

	#[test]
	fn unexpected_json() {
		let node = Node {
			account: Address::new(),
			request_timeout: Duration::from_secs(5),
			poll_interval: Duration::from_secs(1),
			required_confirmations: 0,
			rpc_host: "https://rpc".into(),
			rpc_port: 443,
			password: PathBuf::from("password"),
			info: NodeInfo::default(),
			gas_price_oracle_url: Some("https://gas.price".into()),
			gas_price_speed: GasPriceSpeed::from_str("fast").unwrap(),
			gas_price_timeout: Duration::from_secs(5),
			default_gas_price: 15_000_000_000,
		};
		let timer = Timer::default();
		let mut stream = GasPriceStream::new_with_retriever(&node, UnexpectedJson, &timer);
		loop {
			match stream.poll() {
				Ok(Async::Ready(Some(v))) => {
					assert_eq!(v, node.default_gas_price);
					break;
				},
				Err(_) => panic!("should not error out"),
				_ => (),
			}
		}
	}

	struct NonObjectJson;

	impl Retriever for NonObjectJson {
		type Item = String;
		type Future = FutureResult<Self::Item, Error>;

		fn retrieve(&self, _uri: &Uri) -> <Self as Retriever>::Future {
			ok("3".into())
		}
	}

	#[test]
	fn non_object_json() {
		let node = Node {
			account: Address::new(),
			request_timeout: Duration::from_secs(5),
			poll_interval: Duration::from_secs(1),
			required_confirmations: 0,
			rpc_host: "https://rpc".into(),
			rpc_port: 443,
			password: PathBuf::from("password"),
			info: NodeInfo::default(),
			gas_price_oracle_url: Some("https://gas.price".into()),
			gas_price_speed: GasPriceSpeed::from_str("fast").unwrap(),
			gas_price_timeout: Duration::from_secs(5),
			default_gas_price: 15_000_000_000,
		};
		let timer = Timer::default();
		let mut stream = GasPriceStream::new_with_retriever(&node, NonObjectJson, &timer);
		loop {
			match stream.poll() {
				Ok(Async::Ready(Some(v))) => {
					assert_eq!(v, node.default_gas_price);
					break;
				},
				Err(_) => panic!("should not error out"),
				_ => (),
			}
		}
	}

	struct CorrectJson;

	impl Retriever for CorrectJson {
		type Item = String;
		type Future = FutureResult<Self::Item, Error>;

		fn retrieve(&self, _uri: &Uri) -> <Self as Retriever>::Future {
			ok(r#"{"fast": 12.0}"#.into())
		}
	}

	#[test]
	fn correct_json() {
		let node = Node {
			account: Address::new(),
			request_timeout: Duration::from_secs(5),
			poll_interval: Duration::from_secs(1),
			required_confirmations: 0,
			rpc_host: "https://rpc".into(),
			rpc_port: 443,
			password: PathBuf::from("password"),
			info: NodeInfo::default(),
			gas_price_oracle_url: Some("https://gas.price".into()),
			gas_price_speed: GasPriceSpeed::from_str("fast").unwrap(),
			gas_price_timeout: Duration::from_secs(5),
			default_gas_price: 15_000_000_000,
		};
		let timer = Timer::default();
		let mut stream = GasPriceStream::new_with_retriever(&node, CorrectJson, &timer);
		loop {
			match stream.poll() {
				Ok(Async::Ready(Some(v))) => {
					assert_eq!(v, 12_000_000_000);
					break;
				},
				Err(_) => panic!("should not error out"),
				_ => (),
			}
		}
	}

}

