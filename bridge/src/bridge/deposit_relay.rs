use std::sync::{Arc, RwLock};

use ethabi::{self, RawLog};
use ethcore_transaction::{Action, Transaction};
use futures::{Async, Future, Poll, Stream};
use futures::future::{Join, JoinAll, join_all};
use futures::stream::{Collect, FuturesUnordered, futures_unordered};
use itertools::Itertools;
use tokio_timer::Timeout;
use web3::Transport;
use web3::types::{Address, Bytes, FilterBuilder, Log, U256};

use api::{self, ApiCall, LogStream, LogStreamInit, LogStreamItem, log_stream};
use app::App;
use contracts::{foreign::ForeignBridge, home::HomeBridge};
use database::Database;
use error::{Error, ErrorKind};
use signature::Signature;
use super::BridgeChecked;
use super::nonce::{NonceCheck, SendRawTransaction, send_transaction_with_nonce};
use util::web3_filter;

// A future representing all open calls to the Home contract's
// `message()` and `signature()` functions (with timeouts).
type MessagesAndSignaturesFuture<T: Transport> = Join<
    JoinAll<Vec<Timeout<ApiCall<Bytes, T::Out>>>>,
    JoinAll<Vec<JoinAll<Vec<Timeout<ApiCall<Bytes, T::Out>>>>>>
>;

// A future representing all open calls to the Foreign contract's
// `deposit()` function.
type DepositsFuture<T: Transport> =
    Collect<FuturesUnordered<NonceCheck<T, SendRawTransaction<T>>>>;

// Returns a log filter for the Home Bridge contract's `CollectedSignatures` event.
fn collected_signatures_filter(
    home_contract: &HomeBridge,
    contract_address: Address
) -> FilterBuilder
{
    let filter = home_contract.events().collected_signatures().create_filter();
    web3_filter(filter, contract_address)
}

// Wraps the input data (ie. "payloads") for the Home contract's
// `message()` and `signature()` functions.
struct Payloads {
    message_payload: Bytes,
    signature_payloads: Vec<Bytes>,
}

// Returns the encoded input for the Home Bridge contract's
// `message()` and `signature()` functions.
fn create_message_and_signatures_payloads(
    home_contract: &HomeBridge,
    n_signatures_required: u32,
    my_address: Address,
    collected_signatures_event_log: &Log,
) -> Result<Option<Payloads>, Error>
{
    let tx_hash = collected_signatures_event_log
        .transaction_hash
        .unwrap();
    
    let raw_log = RawLog {
        topics: collected_signatures_event_log.topics.clone(),
        data: collected_signatures_event_log.data.0.clone(),
    };

    let parsed = home_contract.events().collected_signatures()
        .parse_log(raw_log)
        .unwrap();

    let this_bridge_is_responsible_for_relaying_deposit =
        parsed.authority_responsible_for_relay == my_address;

    if this_bridge_is_responsible_for_relaying_deposit {
        info!("this bridge is responsible for relaying Deposit. tx_hash: {}", tx_hash);
    } else {
        info!("this bridge is not responsible for relaying Deposit. tx_hash: {}", tx_hash);
        return Ok(None);
    }

    let message_payload = home_contract.functions().message()
        .input(parsed.message_hash)
        .into();

    let signature_payloads: Vec<Bytes> = (0..n_signatures_required).into_iter()
        .map(|i| home_contract.functions().signature()
            .input(parsed.message_hash, i)
            .into()
        )
        .collect();

    let payloads = Payloads { message_payload, signature_payloads };
    Ok(Some(payloads))
}

// Returns the encoded input (ie. "payload") for the Foreign
// Contract's `deposit()` function.
fn create_deposit_payload(
    foreign_contract: &ForeignBridge, 
    message: Bytes,
    signatures: Vec<Signature>
) -> Vec<u8>
{
    let mut vs = vec![];
    let mut rs = vec![];
    let mut ss = vec![];

    for Signature { v, r, s } in signatures.into_iter() {
        vs.push(v);
        rs.push(r);
        ss.push(s);
    }

    foreign_contract.functions().deposit().input(vs, rs, ss, message.0)
}

// Represents each state in the DepositRelay's state machine.
enum State<T: Transport> {
    // This instance of `DepositRelay` is not waiting for any futures
    // to complete, nor does it have data to yield. The next call to
    // `DepositRelay.poll()` is responsible for querying the Home
    // chain for `CollectedSignatures` event logs.
    Initial,
    // This instance of `DepositRelay` is currently waiting for the
    // futures produced by calling the Home contract's `withdraw()`
    // and `signature()` functions to complete.
    WaitingOnMessagesAndSignatures {
        future: MessagesAndSignaturesFuture<T>,
        last_block_checked: u64,
    },
    // This instance of `DepositRelay` is currently waiting for all
    // calls to the Foreign contract's `deposit()` function to
    // complete.
    WaitingOnDeposits {
        future: DepositsFuture<T>,
        last_block_checked: u64,
    },
    // All futures have completed, yield the last block checked on
    // the Home chain for `CollectedSignatures` events.
    Yield(Option<u64>),
}

// Monitors the Home chain for `CollectedSignatures` events, once
// new CollectedSignatures events are found, the `DepositRelay`
// will call the Foreign Bridge contract's `deposit()` function.
pub struct DepositRelay<T: Transport> {
    app: Arc<App<T>>,
    logs: LogStream<T>,
    state: State<T>,
    foreign_balance: Arc<RwLock<Option<U256>>>,
    foreign_contract_address: Address,
    foreign_chain_id: u64,
    foreign_gas_price: Arc<RwLock<u64>>,
    home_contract_address: Address,
}

pub fn create_deposit_relay<T: Transport>(
    app: Arc<App<T>>,
    init: &Database,
    foreign_balance: Arc<RwLock<Option<U256>>>,
    foreign_chain_id: u64,
    foreign_gas_price: Arc<RwLock<u64>>,
) -> DepositRelay<T>
{ 
    let foreign_contract_address = init.foreign_contract_address;
    let home_contract_address = init.home_contract_address;

	let collected_signatures_event_logs = {
        let last_block_checked = init.checked_deposit_relay;
        let home_conn = app.connections.home.clone(); 
        let home_config = &app.config.home;
        let home_contract = &app.home_bridge;
        let timer = app.timer.clone();
        
        let log_stream_init = LogStreamInit {
            after: last_block_checked,
            request_timeout: home_config.request_timeout,
            poll_interval: home_config.poll_interval,
            confirmations: home_config.required_confirmations,
            filter: collected_signatures_filter(home_contract, home_contract_address),
        };

        log_stream(home_conn, timer, log_stream_init)
    };

	DepositRelay {
		app,
        logs: collected_signatures_event_logs,
		state: State::Initial,
		foreign_balance,
		foreign_contract_address,
        foreign_chain_id,
		foreign_gas_price,
	    home_contract_address,
    }
}

impl<T: Transport> Stream for DepositRelay<T> {
    type Item = BridgeChecked;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> { 
        let app = &self.app;
        let my_home_address = self.app.config.home.account;
        
        let home_config = &self.app.config.home;
        let home_conn = &self.app.connections.home;
        let home_contract = &self.app.home_bridge;
        let home_contract_address = self.home_contract_address;
       
        let foreign_chain_id = self.foreign_chain_id;
        let foreign_config = &self.app.config.foreign;
        let foreign_conn = &self.app.connections.foreign;
        let foreign_contract = &self.app.foreign_bridge;
        let foreign_contract_address = self.foreign_contract_address;

        let gas_per_deposit_call = self.app.config.txs.deposit_relay.gas.into();
        let n_signatures_required = self.app.config.authorities.required_signatures;

        loop {
            let next_state = match self.state {
                State::Initial => {
					let LogStreamItem { to: last_block_checked, logs: collected_signatures_event_logs, .. } =
                        try_stream!(
                            self.logs.poll().map_err(|e| {
                                let context = "polling Home contract for CollectedSignatures event logs";
                                ErrorKind::ContextualizedError(Box::new(e), context)
                            })
                        );

                    let n_new_events = collected_signatures_event_logs.len();
					info!("found {} new CollectedSignatures events", n_new_events);
                
                    let payloads: Vec<Payloads> = collected_signatures_event_logs.iter()
                        .map(|log| create_message_and_signatures_payloads(
                            home_contract,
                            n_signatures_required,
                            my_home_address,
                            log
                        ))
                        .collect::<Result<Vec<Option<Payloads>>, Error>>()?
                        .into_iter()
                        .filter_map(|opt| opt)
                        .collect();

                    let message_calls = payloads.iter()
                        .map(|payload| {
                            let Payloads { message_payload, .. } = payload;
                            app.timer.timeout(
                                api::call(home_conn, home_contract_address, message_payload.clone()),
                                home_config.request_timeout
                            )
                        })
                        .collect();

                    let signature_calls = payloads.iter()
                        .map(|payloads| {
                            let Payloads { signature_payloads, .. } = payloads;
                            let calls = signature_payloads.iter()
                                .map(|signature_payload| app.timer.timeout(
                                    api::call(home_conn, home_contract_address, signature_payload.clone()),
                                    home_config.request_timeout
                                ))
                                .collect();
                            join_all(calls)
                        })
                        .collect();

                    State::WaitingOnMessagesAndSignatures {
                        future: join_all(message_calls).join(join_all(signature_calls)),
                        last_block_checked,
                    }
                },
                State::WaitingOnMessagesAndSignatures { ref mut future, last_block_checked } =>  {
                    let foreign_balance = self.foreign_balance.read().unwrap();

                    if foreign_balance.is_none() {
                        warn!("foreign contract balance is unknown");
                        return Ok(Async::NotReady);
                    }

                    let (messages_raw, signatures_raw) = try_ready!(
                        future.poll().map_err(|e| {
                            let context = "fetching messages and signatures from foreign";
                            ErrorKind::ContextualizedError(Box::new(e), context)
                        })
                    );

                    info!("fetching messages and signatures complete");
                    let n_messages = messages_raw.len();
                    let n_signatures = signatures_raw.len();
                    assert_eq!(n_messages, n_signatures);
                    let n_deposits = U256::from(n_messages);

                    let foreign_gas_price = U256::from(*self.foreign_gas_price.read().unwrap());
                    let balance_required = gas_per_deposit_call * foreign_gas_price * n_deposits;
                    if balance_required > *foreign_balance.as_ref().unwrap() {
                        return Err(ErrorKind::InsufficientFunds.into());
                    }

                    let messages_parsed = messages_raw.iter()
                        .map(|message|
                             home_contract.functions().message()
                                .output(&message.0)
                                .map(Bytes::from)
                        )
                        .collect::<ethabi::Result<Vec<Bytes>>>()
                        .map_err(Error::from)?;

                    let signatures_parsed = signatures_raw.iter()
                        .map(|raw_sigs| {
                            let mut sigs = vec![];
                            
                            for raw_sig in raw_sigs.iter() {
                                let bytes = home_contract.functions().signature().output(&raw_sig.0)?;
                                let sig = Signature::from_bytes(&bytes)?;
                                sigs.push(sig);
                            }

                            Ok(sigs)
                        })
                        .collect::<Result<Vec<Vec<Signature>>, Error>>()?;

                    let deposits = messages_parsed.into_iter()
                        .zip(signatures_parsed.into_iter())
                        .map(|(message, signatures)| {
                            let payload = create_deposit_payload(
                                foreign_contract,
                                message,
                                signatures
                            );

                            let tx = Transaction {
                                gas: gas_per_deposit_call,
                                gas_price: foreign_gas_price,
                                value: U256::zero(),
                                data: payload,
                                nonce: U256::zero(),
                                action: Action::Call(foreign_contract_address),
                            };
                            
                            send_transaction_with_nonce(
                                foreign_conn.clone(),
                                app.clone(),
                                foreign_config.clone(),
                                tx,
                                foreign_chain_id,
                                SendRawTransaction(foreign_conn.clone()),
                            )
                        })
                        .collect_vec();

                    info!("relaying {} deposits", n_deposits);
                    State::WaitingOnDeposits {
                        future: futures_unordered(deposits).collect(),
                        last_block_checked,
                    }
                },
                State::WaitingOnDeposits { ref mut future, last_block_checked } => {
                    let _ = try_ready!(
                        future.poll().map_err(|e| {
                            let context = "relaying deposit to foreign";
                            ErrorKind::ContextualizedError(Box::new(e), context)
                        })
                    );
                    info!("deposit relay completed");
                    State::Yield(Some(last_block_checked))
                },
                State::Yield(ref mut block) => match block.take() {
                    Some(block) => {
                        let checked = BridgeChecked::DepositRelay(block);
                        return Ok(Async::Ready(Some(checked)));
                    },
                    None => State::Initial,
                },
            };

            self.state = next_state;
        }
    }
}
