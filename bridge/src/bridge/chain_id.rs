use futures::{Future, Poll};
use tokio_timer::{Timer, Timeout};
use web3::Transport;
use api::{self, ApiCall};
use error::Error;
use config::Node;

/// State of chain id retrieval
enum ChainIdRetrievalState<T: Transport> {
	/// Chain ID request is waiting to happen
	Wait,
	/// Request is in progress
	ChainIdRequest {
		future: Timeout<ApiCall<String, T::Out>>,
	},
}

pub struct ChainIdRetrieval<T: Transport> {
	transport: T,
	state: ChainIdRetrievalState<T>,
	timer: Timer,
	node: Node,
}

pub fn create_chain_id_retrieval<T: Transport + Clone>(transport: T, node: Node) -> ChainIdRetrieval<T> {
	ChainIdRetrieval {
		state: ChainIdRetrievalState::Wait,
		timer: Timer::default(),
		transport,
		node,
	}
}

impl<T: Transport> Future for ChainIdRetrieval<T> {
	type Item = u64;
	type Error = Error;

	fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
		loop {
			let next_state = match self.state {
				ChainIdRetrievalState::Wait => {
					ChainIdRetrievalState::ChainIdRequest {
						future: self.timer.timeout(api::net_version(&self.transport),
						                           self.node.request_timeout),
					}
				},
				ChainIdRetrievalState::ChainIdRequest { ref mut future } => {
					let value = try_ready!(future.poll());
					let id: u64 = value.parse().unwrap();
					return Ok(id.into());
				},
			};
			self.state = next_state;
		}
	}
}
