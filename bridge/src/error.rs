#![allow(unknown_lints)]

use std::io;
use tokio_timer::{TimerError, TimeoutError};
use {web3, toml, ethabi, rustc_hex};
use ethcore::ethstore;
use ethcore::account_provider::{SignError, Error as AccountError};
use serde_json;
use hyper;

error_chain! {
	types {
		Error, ErrorKind, ResultExt, Result;
	}

	foreign_links {
		Io(io::Error);
		Toml(toml::de::Error);
		Ethabi(ethabi::Error);
		Timer(TimerError);
		Hex(rustc_hex::FromHexError);
		Json(serde_json::Error);
		Hyper(hyper::Error);
	}

	errors {
	    ShutdownRequested
		InsufficientFunds
		// api timeout
		Timeout(request: &'static str) {
			description("Request timeout"),
			display("Request {} timed out", request),
		}
		// workaround for error_chain not allowing to check internal error kind
		// https://github.com/rust-lang-nursery/error-chain/issues/206
		MissingFile(filename: String) {
			description("File not found"),
			display("File {} not found", filename),
		}
		// workaround for lack of web3:Error Display and Error implementations
		Web3(err: web3::Error) {
			description("web3 error"),
			display("{:?}", err),
		}
		KeyStore(err: ethstore::Error) {
		    description("keystore error"),
		    display("keystore error {:?}", err),
		}
		SignError(err: SignError) {
		    description("signing error")
		    display("signing error {:?}", err),
		}
		AccountError(err: AccountError) {
		    description("account error")
		    display("account error {:?}", err),
		}
		ContextualizedError(err: Box<Error>, context: &'static str) {
		    description("contextualized error")
		    display("{:?} in {}", err, context)
		}
		OtherError(error: String) {
		    description("other error")
		    display("{}", error)
		}
	}
}

impl<T> From<TimeoutError<T>> for Error {
	fn from(err: TimeoutError<T>) -> Self {
		match err {
			TimeoutError::Timer(_call, _) | TimeoutError::TimedOut(_call) => {
				ErrorKind::Timeout("communication timeout").into()
			}
		}
	}
}

