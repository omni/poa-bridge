use futures::{Future, Stream, Poll};
use tokio_timer::{Timer, Timeout};
use web3::Transport;
use web3::types::U256;
use api::{self, ApiCall};
use error::Error;
use config::Node;

/// State of balance checking.
enum NonceCheckState<T: Transport> {
	/// Deposit relay is waiting for logs.
	Wait,
	/// Balance request is in progress.
	NonceRequest {
		future: Timeout<ApiCall<U256, T::Out>>,
	},
	/// Balance request completed.
	Yield(Option<U256>),
}

pub struct NonceCheck<T: Transport> {
	transport: T,
	state: NonceCheckState<T>,
	timer: Timer,
	node: Node,
}

use std::fmt::{self, Debug};

impl<T: Transport> Debug for NonceCheck<T> {
	fn fmt(&self, _fmt: &mut fmt::Formatter) -> fmt::Result {
		Ok(())
	}

}

pub fn create_nonce_check<T: Transport + Clone>(transport: T, node: Node) -> NonceCheck<T> {
	NonceCheck {
		state: NonceCheckState::Wait,
		timer: Timer::default(),
		transport,
		node,
	}
}

impl<T: Transport> Stream for NonceCheck<T> {
	type Item = U256;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				NonceCheckState::Wait => {
					NonceCheckState::NonceRequest {
						future: self.timer.timeout(api::eth_get_transaction_count(&self.transport, self.node.account, None),
						                           self.node.request_timeout),
					}
				},
				NonceCheckState::NonceRequest { ref mut future } => {
					let value = try_ready!(future.poll());
					NonceCheckState::Yield(Some(value))
				},
				NonceCheckState::Yield(ref mut nonce) => match  nonce.take() {
					None => NonceCheckState::Wait,
					some => return Ok(some.into()),
				}
			};
			self.state = next_state;
		}
	}
}
