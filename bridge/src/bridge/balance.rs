use futures::{Future, Stream, Poll};
use tokio_timer::{Timer, Timeout};
use web3::Transport;
use web3::types::U256;
use api::{self, ApiCall};
use error::Error;
use config::Node;

/// State of balance checking.
enum BalanceCheckState<T: Transport> {
	/// Deposit relay is waiting for logs.
	Wait,
	/// Balance request is in progress.
	BalanceRequest {
		future: Timeout<ApiCall<U256, T::Out>>,
	},
	/// Balance request completed.
	Yield(Option<U256>),
}

pub struct BalanceCheck<T: Transport> {
	transport: T,
	state: BalanceCheckState<T>,
	timer: Timer,
	node: Node,
}

pub fn create_balance_check<T: Transport + Clone>(transport: T, node: Node) -> BalanceCheck<T> {
	BalanceCheck {
		state: BalanceCheckState::Wait,
		timer: Timer::default(),
		transport,
		node,
	}
}

impl<T: Transport> Stream for BalanceCheck<T> {
	type Item = U256;
	type Error = Error;

	fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
		loop {
			let next_state = match self.state {
				BalanceCheckState::Wait => {
					BalanceCheckState::BalanceRequest {
						future: self.timer.timeout(api::balance(&self.transport, self.node.account, None),
						                           self.node.request_timeout),
					}
				},
				BalanceCheckState::BalanceRequest { ref mut future } => {
					let value = try_ready!(future.poll());
					BalanceCheckState::Yield(Some(value))
				},
				BalanceCheckState::Yield(ref mut balance) => match balance.take() {
					None => BalanceCheckState::Wait,
					some => return Ok(some.into()),
				}
			};
			self.state = next_state;
		}
	}
}
