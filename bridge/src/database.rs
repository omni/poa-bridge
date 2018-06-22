use std::path::Path;
use std::{io, str, fs, fmt};
use std::io::{Read, Write};
use toml;
use error::{Error, ResultExt, ErrorKind};


/// Application "database".
#[derive(Debug, PartialEq, Deserialize, Serialize, Default, Clone)]
pub struct Database {
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

impl str::FromStr for Database {
	type Err = Error;

	/// Returns a new `Database` constructed from the parsed string `s`.
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		toml::from_str(s).chain_err(|| "Cannot parse database toml file/string")
	}
}


impl fmt::Display for Database {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.write_str(&toml::to_string(self).expect("serialization can't fail; qed"))
	}
}

impl Database {
	/// Loads a user defined toml file specified by `path` and returns a new
	/// `Database` containing its parsed contents.
	pub fn load<P: AsRef<Path>>(path: P)
			-> Result<Database, Error> {
		let mut file = match fs::File::open(&path) {
			Ok(file) => file,
			Err(ref err) if err.kind() == io::ErrorKind::NotFound =>
				return Err(ErrorKind::MissingFile(format!("{:?}", path.as_ref())).into()),
			Err(err) => return Err(err).chain_err(|| "Cannot open database file"),
		};

		let mut buffer = String::new();
		file.read_to_string(&mut buffer)?;
		buffer.parse()
	}

	/// Writes a serialized `Database` to a writer.
	pub fn save<W: Write>(&self, mut writer: W) -> Result<(), Error> {
		writer.write_all(self.to_string().as_bytes())?;
		Ok(())
	}
}


#[cfg(test)]
mod tests {
	use super::Database;

	#[test]
	fn database_to_and_from_str() {
		let toml =
r#"home_deploy = 100
foreign_deploy = 101
checked_deposit_relay = 120
checked_withdraw_relay = 121
checked_withdraw_confirm = 121
"#;

		let expected = Database {
			home_deploy: Some(100),
			foreign_deploy: Some(101),
			checked_deposit_relay: 120,
			checked_withdraw_relay: 121,
			checked_withdraw_confirm: 121,
		};

		let database = toml.parse().unwrap();
		assert_eq!(expected, database);
		let s = database.to_string();
		assert!(s.contains(toml));
	}
}
