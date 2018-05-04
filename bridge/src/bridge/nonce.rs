use futures::{Future, Async, Poll, future::{MapErr}};
use tokio_timer::Timeout;
use web3::{self, Transport};
use web3::types::{U256, H256, Bytes};
use ethcore_transaction::Transaction;
use api::{self, ApiCall};
use error::{Error, ErrorKind};
use config::Node;
use transaction::prepare_raw_transaction;
use app::App;
use std::sync::Arc;
use rpc;

/// State of balance checking.
enum NonceCheckState<T: Transport, S: TransactionSender> {
	/// Ready
	Ready,
	/// Ready to request nonce
	Reacquire,
	/// Nonce request is in progress.
	NonceRequest {
		future: Timeout<ApiCall<U256, T::Out>>,
	},
	/// Nonce available
	Nonce(U256),
	/// Transaction is in progress
	TransactionRequest {
		future: Timeout<S::Future>,
	},
}

pub struct NonceCheck<T: Transport, S: TransactionSender> {
	app: Arc<App<T>>,
	transport: T,
	state: NonceCheckState<T, S>,
	node: Node,
	transaction: Transaction,
	chain_id: u64,
	sender: S,
}

use std::fmt::{self, Debug};

impl<T: Transport, S: TransactionSender> Debug for NonceCheck<T, S> {
	fn fmt(&self, _fmt: &mut fmt::Formatter) -> fmt::Result {
		Ok(())
	}

}

pub fn send_transaction_with_nonce<T: Transport + Clone, S: TransactionSender>(transport: T, app: Arc<App<T>>, node: Node, transaction: Transaction, chain_id: u64, sender: S) -> NonceCheck<T, S> {
	NonceCheck {
		app,
		state: NonceCheckState::Ready,
		transport,
		node,
		transaction,
		chain_id,
		sender,
	}
}

impl<T: Transport, S: TransactionSender> Future for NonceCheck<T, S> {
	type Item = S::T;
	type Error = Error;

	fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
		loop {
			let next_state = match self.state {
				NonceCheckState::Ready => {
					let result = NonceCheckState::Nonce(self.node.info.nonce.read().unwrap().clone());
					{
						let mut node_nonce = self.node.info.nonce.write().unwrap();
						let (next, _) = node_nonce.overflowing_add(U256::one());
						*node_nonce = next;
					}
					result
				},
				NonceCheckState::Reacquire => {
					NonceCheckState::NonceRequest {
						future: self.app.timer.timeout(api::eth_get_transaction_count(&self.transport, self.node.account, None),
						                           self.node.request_timeout),
					}
				},
				NonceCheckState::NonceRequest { ref mut future } => {
					let nonce = try_ready!(future.poll());
					let mut node_nonce = self.node.info.nonce.write().unwrap();
					*node_nonce = nonce;
					NonceCheckState::Nonce(nonce)
				},
				NonceCheckState::Nonce(mut nonce) => {
					self.transaction.nonce = nonce;
					match prepare_raw_transaction(self.transaction.clone(), &self.app, &self.node, self.chain_id) {
						Ok(tx) => NonceCheckState::TransactionRequest {
							future: self.app.timer.timeout(self.sender.send(tx), self.node.request_timeout)
						},
						Err(e) => return Err(e),
					}
				},
				NonceCheckState::TransactionRequest { ref mut future } => {
					match future.poll() {
						Ok(Async::Ready(t)) => return Ok(Async::Ready(t)),
						Ok(Async::NotReady) => return Ok(Async::NotReady),
						Err(e) => match e {
							Error(ErrorKind::Web3(web3::error::Error(web3::error::ErrorKind::Rpc(rpc_err), _)), _) => {
								if rpc_err.code == rpc::ErrorCode::ServerError(-32010) && rpc_err.message.ends_with("incrementing the nonce.") {
									// restart the process
									NonceCheckState::Reacquire
								} else if rpc_err.code == rpc::ErrorCode::ServerError(-32010) && rpc_err.message.ends_with("already imported.") {
									let hash = self.transaction.hash(Some(self.chain_id));
									info!("{} already imported on {}, skipping", hash, self.node.rpc_host);
									return Ok(Async::Ready(self.sender.ignore(hash)))
								} else {
									return Err(ErrorKind::Web3(web3::error::ErrorKind::Rpc(rpc_err).into()).into());
								}
							},
							e => return Err(From::from(e)),
						},
					}
				},
			};
			self.state = next_state;
		}
	}
}

pub trait TransactionSender {
	type T;
	type Future : Future<Item = Self::T, Error = Error>;
	fn send(&self, tx: Bytes) -> Self::Future;
	fn ignore(&self, hash: H256) -> Self::T;
}

pub struct SendRawTransaction<T: Transport>(pub T);

impl<T: Transport + Clone> TransactionSender for SendRawTransaction<T> {
	type T = H256;
	type Future = ApiCall<Self::T, T::Out>;

	fn send(&self, tx: Bytes) -> <Self as TransactionSender>::Future {
		api::send_raw_transaction(self.0.clone(), tx)
	}

	fn ignore(&self, hash: H256) -> Self::T {
		hash
	}

}

use std::time::Duration;
pub struct TransactionWithConfirmation<T: Transport>(pub T, pub Duration, pub usize);

use web3::types::TransactionReceipt;

impl<T: Transport + Clone> TransactionSender for TransactionWithConfirmation<T> {
	type T = TransactionReceipt;
	type Future = MapErr<web3::confirm::SendTransactionWithConfirmation<T>, fn(::web3::Error) -> Error>;

	fn send(&self, tx: Bytes) -> <Self as TransactionSender>::Future {
		api::send_raw_transaction_with_confirmation(self.0.clone(), tx, self.1, self.2)
			.map_err( web3_error_to_error)
	}

	fn ignore(&self, hash: H256) -> Self::T {
		let mut receipt = TransactionReceipt::default();
		receipt.transaction_hash = hash;
		receipt
	}

}

fn web3_error_to_error(err: web3::Error) -> Error {
	ErrorKind::Web3(err).into()
}
