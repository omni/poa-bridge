use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::{Async, Future, Poll, Stream};
use hyper::{Chunk, client::HttpConnector, Client, Uri};
use hyper_tls::HttpsConnector;
use serde_json as json;
use tokio_core::reactor::Handle;
use tokio_timer::{Interval, Timer, Timeout};

use config::{GasPriceSpeed, Node};
use error::Error;

const CACHE_TIMEOUT_DURATION: Duration = Duration::from_secs(5 * 60);

enum State {
    Initial,
    WaitingForResponse(Timeout<Box<Future<Item = Chunk, Error = Error>>>),
    Yield(Option<u64>),
}

pub struct GasPriceStream {
    state: State,
    client: Client<HttpsConnector<HttpConnector>>,
    uri: Uri,
    speed: GasPriceSpeed,
    request_timer: Timer,
    interval: Interval,
    last_price: u64,
    request_timeout: Duration,
}

impl GasPriceStream {
    pub fn new(node: &Node, handle: &Handle, timer: &Timer) -> Self {
        let client = Client::configure()
            .connector(HttpsConnector::new(4, handle).unwrap())
            .build(handle);

        let uri: Uri = node.gas_price_oracle_url.clone().unwrap().parse().unwrap();

        GasPriceStream {
            state: State::Initial,
            client,
            uri,
            speed: node.gas_price_speed,
            request_timer: timer.clone(),
            interval: timer.interval_at(Instant::now(), CACHE_TIMEOUT_DURATION),
            last_price: node.default_gas_price,
            request_timeout: node.request_timeout,
        }
    }
}

impl Stream for GasPriceStream {
    type Item = u64;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            let next_state = match self.state {
                State::Initial => {
                    let _ = try_stream!(self.interval.poll());

                    let request: Box<Future<Item = Chunk, Error = Error>> =
                        Box::new(
                            self.client.get(self.uri.clone())
                                .and_then(|resp| resp.body().concat2())
                                .map_err(|e| e.into())
                        );

                    let request_future = self.request_timer
                        .timeout(request, self.request_timeout);

                    State::WaitingForResponse(request_future)
                },
                State::WaitingForResponse(ref mut request_future) => {
                    match request_future.poll() {
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(chunk)) => {
                            match json::from_slice::<HashMap<String, json::Value>>(&chunk) {
								Ok(json_obj) => {
									match json_obj.get(self.speed.as_str()) {
										Some(json::Value::Number(price)) => State::Yield(Some((price.as_f64().unwrap() * 1_000_000_000.0).trunc() as u64)),
										_ => {
											error!("Invalid or missing gas price ({}) in the gas price oracle response: {}", self.speed.as_str(), String::from_utf8_lossy(&*chunk));
											State::Yield(Some(self.last_price))
										},
									}
								},
								Err(e) => {
									error!("Error while parsing response from gas price oracle: {:?} {}", e, String::from_utf8_lossy(&*chunk));
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
