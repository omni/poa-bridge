use ethereum_types::{Address, U256, H256};
use contracts::home::events::Deposit;
use web3::types::Log;
use ethabi::{self, RawLog};
use error::Error;

/// the message that is relayed from side to main.
/// contains all the information required for the relay.
/// validators sign off on this message.
#[derive(PartialEq, Debug)]
pub struct MessageToMainnet {
	pub recipient: Address,
	pub value: U256,
	pub sidenet_transaction_hash: H256,
}

/// length of a `MessageToMainnet.to_bytes()` in bytes
pub const MESSAGE_LENGTH: usize = 84;

impl MessageToMainnet {
	/// parses message from a byte slice
	pub fn from_bytes(bytes: &[u8]) -> Self {
		assert_eq!(bytes.len(), MESSAGE_LENGTH);

		Self {
			recipient: bytes[0..20].into(),
			value: (&bytes[20..52]).into(),
			sidenet_transaction_hash: bytes[52..84].into(),
		}
	}

	/// Creates a message from a `Deposit` event that was logged on Home.
	pub fn from_deposit_log(log: Log) -> Result<Self, Error> {
		let raw_log = RawLog { topics: log.topics, data: log.data.0 };
		let parsed = Deposit::default().parse_log(raw_log)?;
	 
		let tx_hash = log.transaction_hash
			.expect("Deposit event does not contain a `transaction_hash`");

		let msg = MessageToMainnet {
			recipient: parsed.recipient,
			value: parsed.value,
			sidenet_transaction_hash: tx_hash,
		};

		Ok(msg)
	}

	/// serializes message to a byte vector.
	/// mainly used to construct the message byte vector that is then signed
	/// and passed to `ForeignBridge.submitSignature`
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut result = vec![0u8; MESSAGE_LENGTH];
		result[0..20].copy_from_slice(&self.recipient.0[..]);
		result[20..52].copy_from_slice(&H256::from(self.value));
		result[52..].copy_from_slice(&self.sidenet_transaction_hash.0[..]);
		return result;
	}

	/// serializes message to an ethabi payload
	pub fn to_payload(&self) -> Vec<u8> {
		ethabi::encode(&[ethabi::Token::Bytes(self.to_bytes())])
	}
}

#[cfg(test)]
mod test {
	use quickcheck::TestResult;
	use super::*;

	quickcheck! {
		fn quickcheck_message_to_mainnet_roundtrips_to_bytes(
			recipient_raw: Vec<u8>,
			value_raw: u64,
			sidenet_transaction_hash_raw: Vec<u8>
		) -> TestResult {
			if recipient_raw.len() != 20 || sidenet_transaction_hash_raw.len() != 32 {
				return TestResult::discard();
			}

			let recipient: Address = recipient_raw.as_slice().into();
			let value: U256 = value_raw.into();
			let sidenet_transaction_hash: H256 = sidenet_transaction_hash_raw.as_slice().into();

			let message = MessageToMainnet {
				recipient,
				value,
				sidenet_transaction_hash,
			};

			let bytes = message.to_bytes();
			assert_eq!(message, MessageToMainnet::from_bytes(bytes.as_slice()));

			let payload = message.to_payload();
			let mut tokens = ethabi::decode(&[ethabi::ParamType::Bytes], payload.as_slice())
				.unwrap();
			let decoded = tokens.pop().unwrap().to_bytes().unwrap();
			assert_eq!(message, MessageToMainnet::from_bytes(decoded.as_slice()));

			TestResult::passed()
		}
	}
}
