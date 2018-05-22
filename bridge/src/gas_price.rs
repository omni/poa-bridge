use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::{Async, future::AndThen, Future, Poll, stream::Concat2, Stream};
use hyper::{Body, Chunk, client::{FutureResponse, HttpConnector}, Client, Response, Uri};
use hyper_tls::HttpsConnector;
use serde_json as json;
use tokio_core::reactor::Handle;
use tokio_timer::{Interval, Timer, Timeout};

use config::{GasPriceSpeed, Node};
use error::Error;

const CACHE_TIMEOUT_DURATION: Duration = Duration::from_secs(5 * 60);
const REQUEST_TIMEOUT_DURATION: Duration = Duration::from_secs(30);

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
    default_price: u64,
    request_timer: Timer,
    interval: Interval,
}

impl GasPriceStream {
    pub fn new(node: &Node, handle: &Handle, timer: &Timer) -> Self {
        let client = Client::configure()
            .connector(HttpsConnector::new(4, handle).unwrap())
            .build(handle);

        GasPriceStream {
            state: State::Initial,
            client,
            uri: node.gas_price_oracle_url.parse().unwrap(),
            speed: node.gas_price_speed,
            default_price: node.default_gas_price,
            request_timer: timer.clone(),
            interval: timer.interval_at(Instant::now(), CACHE_TIMEOUT_DURATION),
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
                        .timeout(request, REQUEST_TIMEOUT_DURATION);

                    State::WaitingForResponse(request_future)
                },
                State::WaitingForResponse(ref mut request_future) => {
                    match request_future.poll() {
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(chunk)) => {
                            let json_obj: HashMap<String, json::Value> = json::from_slice(&chunk)?;

                            let gas_price = match json_obj.get(self.speed.as_str()) {
                                Some(json::Value::Number(price)) => (price.as_f64().unwrap() * 1_000_000_000.0).trunc() as u64,
                                _ => unreachable!(),
                            };

                            State::Yield(Some(gas_price))
                        },
                        Err(e) => panic!(e), 
                    }
                },
                State::Yield(ref mut opt) => match opt.take() {
					None => State::Initial,
					price => return Ok(Async::Ready(price)),
				}
            };

            self.state = next_state;
        }
    }
}

