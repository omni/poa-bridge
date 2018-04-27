use error::{Error, ErrorKind};
use ethcore_transaction::{Transaction, SignedTransaction};
use web3::types::Bytes;
use config::Node;
use app::App;
use web3::Transport;

pub fn prepare_raw_transaction<T: Transport>(tx: Transaction, app: &App<T>, node: &Node, chain_id: u64) -> Result<Bytes, Error> {
	let hash = tx.hash(Some(chain_id));

	let sig = app.keystore.sign(node.account, None, hash).map_err(|e| ErrorKind::SignError(e))?;
	let tx = SignedTransaction::new(tx.with_signature(sig, Some(chain_id))).unwrap();

	use rlp::{RlpStream, Encodable};
	let mut stream = RlpStream::new();
	tx.rlp_append(&mut stream);

	Ok(Bytes(stream.out()))
}
