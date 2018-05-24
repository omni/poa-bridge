use std::path::{Path, PathBuf};
use std::fs;
use std::io::Read;
use std::str::FromStr;
use std::time::Duration;
#[cfg(feature = "deploy")]
use rustc_hex::FromHex;
use web3::types::Address;
#[cfg(feature = "deploy")]
use web3::types::Bytes;
use error::{ResultExt, Error};
use {toml};

const DEFAULT_POLL_INTERVAL: u64 = 1;
const DEFAULT_CONFIRMATIONS: usize = 12;
const DEFAULT_TIMEOUT: u64 = 3600;
const DEFAULT_RPC_PORT: u16 = 8545;
const DEFAULT_CONCURRENCY: usize = 100;
const DEFAULT_GAS_PRICE_SPEED: GasPriceSpeed = GasPriceSpeed::Fast;
const DEFAULT_GAS_PRICE_TIMEOUT_SECS: u64 = 10;
const DEFAULT_GAS_PRICE_WEI: u64 = 15_000_000_000;

/// Application config.
#[derive(Debug, PartialEq, Clone)]
pub struct Config {
	pub home: Node,
	pub foreign: Node,
	pub authorities: Authorities,
	pub txs: Transactions,
	#[cfg(feature = "deploy")]
	pub estimated_gas_cost_of_withdraw: u32,
	pub keystore: PathBuf,
}

impl Config {
	pub fn load<P: AsRef<Path>>(path: P) -> Result<Config, Error> {
		let mut file = fs::File::open(path).chain_err(|| "Cannot open config")?;
		let mut buffer = String::new();
		file.read_to_string(&mut buffer).expect("TODO");
		Self::load_from_str(&buffer)
	}

	fn load_from_str(s: &str) -> Result<Config, Error> {
		let config: load::Config = toml::from_str(s).chain_err(|| "Cannot parse config")?;
		Config::from_load_struct(config)
	}

	fn from_load_struct(config: load::Config) -> Result<Config, Error> {
		let result = Config {
			home: Node::from_load_struct(config.home)?,
			foreign: Node::from_load_struct(config.foreign)?,
			authorities: Authorities {
				accounts: config.authorities.accounts,
				required_signatures: config.authorities.required_signatures,
			},
			txs: config.transactions.map(Transactions::from_load_struct).unwrap_or_default(),
			#[cfg(feature = "deploy")]
			estimated_gas_cost_of_withdraw: config.estimated_gas_cost_of_withdraw,
			keystore: config.keystore,
		};

		Ok(result)
	}
}

#[derive(Debug, PartialEq, Clone)]
pub struct Node {
	pub account: Address,
	#[cfg(feature = "deploy")]
	pub contract: ContractConfig,
	pub request_timeout: Duration,
	pub poll_interval: Duration,
	pub required_confirmations: usize,
	pub rpc_host: String,
	pub rpc_port: u16,
	pub password: PathBuf,
	pub info: NodeInfo,
	pub gas_price_oracle_url: Option<String>,
	pub gas_price_speed: GasPriceSpeed,
	pub gas_price_timeout: Duration,
	pub default_gas_price: u64,
}

use std::sync::{Arc, RwLock};
use web3::types::U256;

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub nonce: Arc<RwLock<U256>>,
}

impl Default for NodeInfo {
	fn default() -> Self {
		NodeInfo {
			nonce: Arc::new(RwLock::new(U256::zero())),
		}
	}
}

impl PartialEq for NodeInfo {
	fn eq(&self, rhs: &Self) -> bool {
		*self.nonce.read().unwrap() == *rhs.nonce.read().unwrap()
	}
}

impl Node {
	fn from_load_struct(node: load::Node) -> Result<Node, Error> {
		let gas_price_oracle_url = node.gas_price_oracle_url.clone();
		
		let gas_price_speed = match node.gas_price_speed {
			Some(ref s) => GasPriceSpeed::from_str(s).unwrap(),
			None => DEFAULT_GAS_PRICE_SPEED
		};

		let gas_price_timeout = {
			let n_secs = node.gas_price_timeout.unwrap_or(DEFAULT_GAS_PRICE_TIMEOUT_SECS);
			Duration::from_secs(n_secs)
		};

		let default_gas_price = node.default_gas_price.unwrap_or(DEFAULT_GAS_PRICE_WEI);

		let result = Node {
			account: node.account,
			#[cfg(feature = "deploy")]
			contract: ContractConfig {
				bin: {
					let mut read = String::new();
					let mut file = fs::File::open(node.contract.bin)?;
					file.read_to_string(&mut read)?;
					Bytes(read.from_hex()?)
				}
			},
			request_timeout: Duration::from_secs(node.request_timeout.unwrap_or(DEFAULT_TIMEOUT)),
			poll_interval: Duration::from_secs(node.poll_interval.unwrap_or(DEFAULT_POLL_INTERVAL)),
			required_confirmations: node.required_confirmations.unwrap_or(DEFAULT_CONFIRMATIONS),
			rpc_host: node.rpc_host.unwrap(),
			rpc_port: node.rpc_port.unwrap_or(DEFAULT_RPC_PORT),
			password: node.password,
			info: Default::default(),
			gas_price_oracle_url,
			gas_price_speed,
			gas_price_timeout,
			default_gas_price,
		};

		Ok(result)
	}

	pub fn password(&self) -> Result<String, Error> {
		use std::io::Read;
		use std::fs;
		let mut f = fs::File::open(&self.password)?;
		let mut s = String::new();
		f.read_to_string(&mut s)?;
		Ok(s.split("\n").next().unwrap().to_string())
	}
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Transactions {
	#[cfg(feature = "deploy")]
	pub home_deploy: TransactionConfig,
	#[cfg(feature = "deploy")]
	pub foreign_deploy: TransactionConfig,
	pub deposit_relay: TransactionConfig,
	pub withdraw_confirm: TransactionConfig,
	pub withdraw_relay: TransactionConfig,
}

impl Transactions {
	fn from_load_struct(cfg: load::Transactions) -> Self {
		Transactions {
			#[cfg(feature = "deploy")]
			home_deploy: cfg.home_deploy.map(TransactionConfig::from_load_struct).unwrap_or_default(),
			#[cfg(feature = "deploy")]
			foreign_deploy: cfg.foreign_deploy.map(TransactionConfig::from_load_struct).unwrap_or_default(),
			deposit_relay: cfg.deposit_relay.map(TransactionConfig::from_load_struct).unwrap_or_default(),
			withdraw_confirm: cfg.withdraw_confirm.map(TransactionConfig::from_load_struct).unwrap_or_default(),
			withdraw_relay: cfg.withdraw_relay.map(TransactionConfig::from_load_struct).unwrap_or_default(),
		}
	}
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct TransactionConfig {
	pub gas: u64,
	pub gas_price: u64,
	pub concurrency: usize,
}

impl TransactionConfig {
	fn from_load_struct(cfg: load::TransactionConfig) -> Self {
		TransactionConfig {
			gas: cfg.gas.unwrap_or_default(),
			gas_price: cfg.gas_price.unwrap_or_default(),
			concurrency: cfg.concurrency.unwrap_or(DEFAULT_CONCURRENCY),
		}
	}
}

#[cfg(feature = "deploy")]
#[derive(Debug, PartialEq, Clone)]
pub struct ContractConfig {
	pub bin: Bytes,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Authorities {
	pub accounts: Vec<Address>,
	pub required_signatures: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GasPriceSpeed {
    Instant,
    Fast,
    Standard,
    Slow,
}

impl FromStr for GasPriceSpeed {
	type Err = ();
	
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let speed = match s {
			"instant" => GasPriceSpeed::Instant,
			"fast" => GasPriceSpeed::Fast,
			"standard" => GasPriceSpeed::Standard,
			"slow" => GasPriceSpeed::Slow,
			_ => return Err(()),
		};
		Ok(speed)
	}
}

impl GasPriceSpeed {
	pub fn as_str(&self) -> &str {
		match *self {
			GasPriceSpeed::Instant => "instant",
			GasPriceSpeed::Fast => "fast",
			GasPriceSpeed::Standard => "standard",
			GasPriceSpeed::Slow => "slow",
		}
	}
}

/// Some config values may not be defined in `toml` file, but they should be specified at runtime.
/// `load` module separates `Config` representation in file with optional from the one used
/// in application.
mod load {
	use std::path::PathBuf;
	use web3::types::Address;

	#[derive(Deserialize)]
	pub struct Config {
		pub home: Node,
		pub foreign: Node,
		pub authorities: Authorities,
		pub transactions: Option<Transactions>,
		#[cfg(feature = "deploy")]
		pub estimated_gas_cost_of_withdraw: u32,
		pub keystore: PathBuf,
	}

	#[derive(Deserialize)]
	pub struct Node {
		pub account: Address,
		#[cfg(feature = "deploy")]
		pub contract: ContractConfig,
		pub request_timeout: Option<u64>,
		pub poll_interval: Option<u64>,
		pub required_confirmations: Option<usize>,
		pub rpc_host: Option<String>,
		pub rpc_port: Option<u16>,
		pub password: PathBuf,
		pub gas_price_oracle_url: Option<String>,
		pub gas_price_speed: Option<String>,
		pub gas_price_timeout: Option<u64>,
		pub default_gas_price: Option<u64>,
	}

	#[derive(Deserialize)]
	pub struct Transactions {
		#[cfg(feature = "deploy")]
		pub home_deploy: Option<TransactionConfig>,
		#[cfg(feature = "deploy")]
		pub foreign_deploy: Option<TransactionConfig>,
		pub deposit_relay: Option<TransactionConfig>,
		pub withdraw_confirm: Option<TransactionConfig>,
		pub withdraw_relay: Option<TransactionConfig>,
	}

	#[derive(Deserialize)]
	#[serde(deny_unknown_fields)]
	pub struct TransactionConfig {
		pub gas: Option<u64>,
		pub gas_price: Option<u64>,
		pub concurrency: Option<usize>,
	}

	#[derive(Deserialize)]
	#[serde(deny_unknown_fields)]
	pub struct ContractConfig {
		pub bin: PathBuf,
	}

	#[derive(Deserialize)]
	#[serde(deny_unknown_fields)]
	pub struct Authorities {
		pub accounts: Vec<Address>,
		pub required_signatures: u32,
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;
	#[cfg(feature = "deploy")]
	use rustc_hex::FromHex;
	use super::{Config, Node, Transactions, Authorities};
	#[cfg(feature = "deploy")]
	use super::ContractConfig;
	#[cfg(feature = "deploy")]
    use super::TransactionConfig;
	use super::{DEFAULT_TIMEOUT, DEFAULT_CONCURRENCY, DEFAULT_GAS_PRICE_SPEED, DEFAULT_GAS_PRICE_TIMEOUT_SECS, DEFAULT_GAS_PRICE_WEI};

	#[test]
	fn load_full_setup_from_str() {
		let toml = r#"
keystore = "/keys"
estimated_gas_cost_of_withdraw = 100000

[home]
account = "0x1B68Cb0B50181FC4006Ce572cF346e596E51818b"
poll_interval = 2
required_confirmations = 100
rpc_host = "127.0.0.1"
rpc_port = 8545
password = "password"

[home.contract]
bin = "../compiled_contracts/HomeBridge.bin"

[foreign]
account = "0x0000000000000000000000000000000000000001"
rpc_host = "127.0.0.1"
rpc_port = 8545
password = "password"

[foreign.contract]
bin = "../compiled_contracts/ForeignBridge.bin"

[authorities]
accounts = [
	"0x0000000000000000000000000000000000000001",
	"0x0000000000000000000000000000000000000002",
	"0x0000000000000000000000000000000000000003"
]
required_signatures = 2

[transactions]
home_deploy = { gas = 20 }
"#;

		#[allow(unused_mut)]
		let mut expected = Config {
			txs: Transactions::default(),
			home: Node {
				account: "1B68Cb0B50181FC4006Ce572cF346e596E51818b".into(),
				#[cfg(feature = "deploy")]
				contract: ContractConfig {
					bin: include_str!("../../compiled_contracts/HomeBridge.bin").from_hex().unwrap().into(),
				},
				poll_interval: Duration::from_secs(2),
				request_timeout: Duration::from_secs(DEFAULT_TIMEOUT),
				required_confirmations: 100,
				rpc_host: "127.0.0.1".into(),
				rpc_port: 8545,
				password: "password".into(),
				info: Default::default(),
				gas_price_oracle_url: None,
				gas_price_speed: DEFAULT_GAS_PRICE_SPEED,
				gas_price_timeout: Duration::from_secs(DEFAULT_GAS_PRICE_TIMEOUT_SECS),
				default_gas_price: DEFAULT_GAS_PRICE_WEI,
			},
			foreign: Node {
				account: "0000000000000000000000000000000000000001".into(),
				#[cfg(feature = "deploy")]
				contract: ContractConfig {
					bin: include_str!("../../compiled_contracts/ForeignBridge.bin").from_hex().unwrap().into(),
				},
				poll_interval: Duration::from_secs(1),
				request_timeout: Duration::from_secs(DEFAULT_TIMEOUT),
				required_confirmations: 12,
				rpc_host: "127.0.0.1".into(),
				rpc_port: 8545,
				password: "password".into(),
				info: Default::default(),
				gas_price_oracle_url: None,
				gas_price_speed: DEFAULT_GAS_PRICE_SPEED,
				gas_price_timeout: Duration::from_secs(DEFAULT_GAS_PRICE_TIMEOUT_SECS),
				default_gas_price: DEFAULT_GAS_PRICE_WEI,
			},
			authorities: Authorities {
				accounts: vec![
					"0000000000000000000000000000000000000001".into(),
					"0000000000000000000000000000000000000002".into(),
					"0000000000000000000000000000000000000003".into(),
				],
				required_signatures: 2,
			},
			#[cfg(feature = "deploy")]
			estimated_gas_cost_of_withdraw: 100_000,
			keystore: "/keys/".into(),
		};

		#[cfg(feature = "deploy")] {
			expected.txs.home_deploy = TransactionConfig {
				gas: 20,
				gas_price: 0,
				concurrency: DEFAULT_CONCURRENCY,
			};
		}

		let config = Config::load_from_str(toml).unwrap();
		assert_eq!(expected, config);
	}

	#[test]
	fn load_minimal_setup_from_str() {
		let toml = r#"
keystore = "/keys/"
estimated_gas_cost_of_withdraw = 200000000

[home]
account = "0x1B68Cb0B50181FC4006Ce572cF346e596E51818b"
rpc_host = ""
password = "password"

[home.contract]
bin = "../compiled_contracts/HomeBridge.bin"

[foreign]
account = "0x0000000000000000000000000000000000000001"
rpc_host = ""
password = "password"

[foreign.contract]
bin = "../compiled_contracts/ForeignBridge.bin"

[authorities]
accounts = [
	"0x0000000000000000000000000000000000000001",
	"0x0000000000000000000000000000000000000002",
	"0x0000000000000000000000000000000000000003"
]
required_signatures = 2
"#;
		let expected = Config {
			txs: Transactions::default(),
			home: Node {
				account: "1B68Cb0B50181FC4006Ce572cF346e596E51818b".into(),
				#[cfg(feature = "deploy")]
				contract: ContractConfig {
					bin: include_str!("../../compiled_contracts/HomeBridge.bin").from_hex().unwrap().into(),
				},
				poll_interval: Duration::from_secs(1),
				request_timeout: Duration::from_secs(DEFAULT_TIMEOUT),
				required_confirmations: 12,
				rpc_host: "".into(),
				rpc_port: 8545,
				password: "password".into(),
				info: Default::default(),
				gas_price_oracle_url: None,
				gas_price_speed: DEFAULT_GAS_PRICE_SPEED,
				gas_price_timeout: Duration::from_secs(DEFAULT_GAS_PRICE_TIMEOUT_SECS),
				default_gas_price: DEFAULT_GAS_PRICE_WEI,
			},
			foreign: Node {
				account: "0000000000000000000000000000000000000001".into(),
				#[cfg(feature = "deploy")]
				contract: ContractConfig {
					bin: include_str!("../../compiled_contracts/ForeignBridge.bin").from_hex().unwrap().into(),
				},
				poll_interval: Duration::from_secs(1),
				request_timeout: Duration::from_secs(DEFAULT_TIMEOUT),
				required_confirmations: 12,
				rpc_host: "".into(),
				rpc_port: 8545,
				password: "password".into(),
				info: Default::default(),
				gas_price_oracle_url: None,
				gas_price_speed: DEFAULT_GAS_PRICE_SPEED,
				gas_price_timeout: Duration::from_secs(DEFAULT_GAS_PRICE_TIMEOUT_SECS),
				default_gas_price: DEFAULT_GAS_PRICE_WEI,
			},
			authorities: Authorities {
				accounts: vec![
					"0000000000000000000000000000000000000001".into(),
					"0000000000000000000000000000000000000002".into(),
					"0000000000000000000000000000000000000003".into(),
				],
				required_signatures: 2,
			},
			#[cfg(feature = "deploy")]
			estimated_gas_cost_of_withdraw: 200_000_000,
			keystore: "/keys/".into(),
		};

		let config = Config::load_from_str(toml).unwrap();
		assert_eq!(expected, config);
	}
}
