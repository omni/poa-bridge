use std::path::Path;
use std::{io, str, fs, fmt};
use std::io::{Read, Write};
use web3::types::Address;
use toml;
use error::{Error, ResultExt, ErrorKind};


/// Application "database".
#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct Database {
	/// Address of home contract.
	pub home_contract_address: Address,
	/// Address of foreign contract.
	pub foreign_contract_address: Address,
	/// Number of block at which home contract has been deployed.
	pub home_deploy: Option<u64>,
	/// Number of block at which foreign contract has been deployed.
	pub foreign_deploy: Option<u64>,
	/// Number of last block which has been checked for deposit relays.
	pub checked_deposit_relay: u64,
	/// Number of last block which has been checked for withdraw relays.
	pub checked_withdraw_relay: u64,
	/// Number of last block which has been checked for withdraw confirms.
	pub checked_withdraw_confirm: u64,
}

impl From<parsed::StoredDatabase> for Database {
	fn from(db_parsed: parsed::StoredDatabase) -> Database {
		Database {
			home_contract_address: db_parsed.home_contract_address,
			foreign_contract_address: db_parsed.foreign_contract_address,
			home_deploy: db_parsed.home_deploy,
			foreign_deploy: db_parsed.foreign_deploy,
			checked_deposit_relay: db_parsed.checked_deposit_relay,
			checked_withdraw_relay: db_parsed.checked_withdraw_relay,
			checked_withdraw_confirm: db_parsed.checked_withdraw_confirm,
		}
	}
}

impl fmt::Display for Database {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.write_str(&toml::to_string(self).expect("serialization can't fail; qed"))
	}
}

impl Database {
	/// Loads the toml file specified by `path` and returns a new `Database`
	/// containing its parsed contents.
	#[deprecated(note = "Use '::load_stored' instead.")]
	pub fn load<P: AsRef<Path>>(path: P) -> Result<Database, Error> {
		Self::load_stored(path)
	}

	/// Loads the toml file specified by `path` and returns a new `Database`
	/// containing its parsed contents.
	pub fn load_stored<P: AsRef<Path>>(path: P) -> Result<Database, Error> {
		let mut file = match fs::File::open(&path) {
			Ok(file) => file,
			Err(ref err) if err.kind() == io::ErrorKind::NotFound =>
				return Err(ErrorKind::MissingFile(format!("{:?}", path.as_ref())).into()),
			Err(err) => return Err(err).chain_err(|| "Cannot open database file"),
		};

		let mut buffer = String::new();
		file.read_to_string(&mut buffer)?;
		Ok(toml::from_str::<parsed::StoredDatabase>(&buffer)?.into())

	}

	/// Loads a user defined toml file specified by `path` and returns a new
	/// `Database` containing its parsed contents.
	pub fn load_user_defined<P: AsRef<Path>>(path: P, home_contract_address: Address,
			foreign_contract_address: Address)
			-> Result<Database, Error> {
		let mut file = match fs::File::open(&path) {
			Ok(file) => file,
			Err(ref err) if err.kind() == io::ErrorKind::NotFound =>
				return Err(ErrorKind::MissingFile(format!("{:?}", path.as_ref())).into()),
			Err(err) => return Err(err).chain_err(|| "Cannot open database file"),
		};

		let mut buffer = String::new();
		file.read_to_string(&mut buffer)?;
		Database::from_str_user_defined(&buffer, home_contract_address, foreign_contract_address)
	}

	/// Returns a new `Database` constructed from a stored toml string
	/// containing keys for 'home_contract_address' and
	/// 'foreign_contract_address'.
	#[cfg(test)]
	fn from_str_stored<S: AsRef<str>>(s: S) -> Result<Database, Error> {
		toml::from_str::<parsed::StoredDatabase>(s.as_ref())
			.map(Database::from)
			.map_err(Error::from)
	}

	/// Returns a new `Database` constructed from the parsed string `s` and
	/// the provided addresses.
	///
	/// An error will be returned if the `s` contains keys for
	/// 'home_contract_address' or 'foreign_contract_address'.
	fn from_str_user_defined<S: AsRef<str>>(s: S, home_contract_address: Address,
			foreign_contract_address: Address) -> Result<Database, Error> {
		let db_parsed: parsed::UserDefinedDatabase = toml::from_str(s.as_ref())
			.chain_err(|| "Cannot parse database file")?;

		Ok(Database {
			home_contract_address,
			foreign_contract_address,
			home_deploy: db_parsed.home_deploy,
			foreign_deploy: db_parsed.foreign_deploy,
			checked_deposit_relay: db_parsed.checked_deposit_relay,
			checked_withdraw_relay: db_parsed.checked_withdraw_relay,
			checked_withdraw_confirm: db_parsed.checked_withdraw_confirm,
		})
	}

	/// Writes a serialized `Database` to a writer.
	#[deprecated(note = "Use '::store' instead.")]
	pub fn save<W: Write>(&self, writer: W) -> Result<(), Error> {
		self.store(writer)
	}

	/// Writes a serialized `Database` to a writer.
	pub fn store<W: Write>(&self, mut writer: W) -> Result<(), Error> {
		writer.write_all(self.to_string().as_bytes())?;
		Ok(())
	}
}

mod parsed {
	#[cfg(test)]
	use std::fmt;
	#[cfg(test)]
	use toml;

	use super::Address;

	/// Parsed application "database".
	#[derive(Debug, Deserialize, Serialize)]
	#[serde(deny_unknown_fields)]
	pub struct StoredDatabase {
		/// Address of home contract.
		pub home_contract_address: Address,
		/// Address of foreign contract.
		pub foreign_contract_address: Address,
		/// Number of block at which home contract has been deployed.
		pub home_deploy: Option<u64>,
		/// Number of block at which foreign contract has been deployed.
		pub foreign_deploy: Option<u64>,
		/// Number of last block which has been checked for deposit relays.
		pub checked_deposit_relay: u64,
		/// Number of last block which has been checked for withdraw relays.
		pub checked_withdraw_relay: u64,
		/// Number of last block which has been checked for withdraw confirms.
		pub checked_withdraw_confirm: u64,
	}

	#[cfg(test)]
	impl fmt::Display for StoredDatabase {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			f.write_str(&toml::to_string(self).expect("serialization can't fail; qed"))
		}
	}

	/// Parsed application "database".
	#[derive(Debug, Deserialize, Serialize)]
	#[serde(deny_unknown_fields)]
	pub struct UserDefinedDatabase {
		/// Number of block at which home contract has been deployed.
		pub home_deploy: Option<u64>,
		/// Number of block at which foreign contract has been deployed.
		pub foreign_deploy: Option<u64>,
		/// Number of last block which has been checked for deposit relays.
		pub checked_deposit_relay: u64,
		/// Number of last block which has been checked for withdraw relays.
		pub checked_withdraw_relay: u64,
		/// Number of last block which has been checked for withdraw confirms.
		pub checked_withdraw_confirm: u64,
	}

	#[cfg(test)]
	impl fmt::Display for UserDefinedDatabase {
		fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
			f.write_str(&toml::to_string(self).expect("serialization can't fail; qed"))
		}
	}
}


#[cfg(test)]
mod tests {
	use super::Database;

	#[test]
	fn database_to_and_from_str_stored() {
		let toml =
r#"home_contract_address = "0x49edf201c1e139282643d5e7c6fb0c7219ad1db7"
foreign_contract_address = "0x49edf201c1e139282643d5e7c6fb0c7219ad1db8"
home_deploy = 100
foreign_deploy = 101
checked_deposit_relay = 120
checked_withdraw_relay = 121
checked_withdraw_confirm = 121
"#;

		let expected = Database {
			home_contract_address: "49edf201c1e139282643d5e7c6fb0c7219ad1db7".into(),
			foreign_contract_address: "49edf201c1e139282643d5e7c6fb0c7219ad1db8".into(),
			home_deploy: Some(100),
			foreign_deploy: Some(101),
			checked_deposit_relay: 120,
			checked_withdraw_relay: 121,
			checked_withdraw_confirm: 121,
		};

		let database = Database::from_str_stored(toml).unwrap();
		assert_eq!(expected, database);
		let s = database.to_string();
		assert_eq!(s, toml);
	}

	#[test]
	fn database_to_and_from_str_user_defined() {
		let toml =
r#"home_deploy = 100
foreign_deploy = 101
checked_deposit_relay = 120
checked_withdraw_relay = 121
checked_withdraw_confirm = 121
"#;

		let home_contract_address = "49edf201c1e139282643d5e7c6fb0c7219ad1db7".into();
		let foreign_contract_address = "49edf201c1e139282643d5e7c6fb0c7219ad1db8".into();

		let expected = Database {
			home_contract_address,
			foreign_contract_address,
			home_deploy: Some(100),
			foreign_deploy: Some(101),
			checked_deposit_relay: 120,
			checked_withdraw_relay: 121,
			checked_withdraw_confirm: 121,
		};

		let database = Database::from_str_user_defined(toml,
			home_contract_address, foreign_contract_address).unwrap();
		assert_eq!(expected, database);
		let s = database.to_string();
		assert!(s.contains(toml));
	}
}
