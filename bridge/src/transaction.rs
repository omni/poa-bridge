use error::Error;
use ethcore_transaction::{Transaction, SignedTransaction};
use web3::types::Bytes;
use config::Node;
use app::App;
use web3::Transport;
use ethcore::ethstore::SimpleSecretStore;

pub fn prepare_raw_transaction<T: Transport, A: AsRef<App<T>>>(tx: Transaction, app: A, node: &Node, chain_id: u64) -> Result<Bytes, Error> {
	let hash = tx.hash(Some(chain_id));

	let account = app.as_ref().keystore.account_ref(&node.account).unwrap();
	let sig = app.as_ref().keystore.sign(&account, &node.password()?, &hash).unwrap();

	let tx = SignedTransaction::new(tx.with_signature(sig, Some(chain_id))).unwrap();

	use rlp::{RlpStream, Encodable};
	let mut stream = RlpStream::new();
	tx.rlp_append(&mut stream);

	Ok(Bytes(stream.out()))
}
