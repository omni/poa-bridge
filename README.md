# POA-Ethereum Bridge

[![Join the chat at https://gitter.im/poanetwork/poa-bridge](https://badges.gitter.im/poanetwork/poa-bridge.svg)](https://gitter.im/poanetwork/poa-bridge?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge&utm_content=badge)
[![Waffle.io - Columns and their card count](https://badge.waffle.io/poanetwork/poa-bridge.svg?columns=all)](https://waffle.io/poanetwork/poa-bridge)

This is software to be operated by *POA bridge validators* to faciliate proof-of-authority
based briding of POA to tokens on an **another** Ethereum-based blockchain.

The validators work with POA bridge contracts to convert ether on one chain into the same
amount of ERC20 tokens on the other and back.

This software works in conjunction with other projects:

* [POA Bridge UI](https://github.com/poanetwork/bridge-ui)
* [POA Bridge Smart Contracts](https://github.com/poanetwork/poa-bridge-contracts)
* [POA Bridge Monitoring service](https://github.com/poanetwork/bridge-monitor)
* [POA Bridge Deployment scripts](https://github.com/poanetwork/deployment-bridge)

### Functionality

The bridge connects two chains (`home` and `foreign`). When a user deposits ether into the
bridge contract contract on `home` they get the same amount of ERC20 tokens on `foreign`,
and they can convert them back as well.

#### Deposit

![deposit](./res/deposit.png)

#### Withdraw

![withdraw](./res/withdraw.png)

### Difference from Parity Bridge

Although POA bridge initially was based on [Parity Brigde](https://github.com/paritytech/parity-bridge), eventually it
was re-worked:
  * support of a gas price oracle introduced;
  * RPC is used instead of IPC;
  * sending of bridge approvals enhanced as so performance increased drammatically;
  * error handling improved to be compatible with Linux systemd faclity;
  * bridge configuration parameters are being got from bridge contracts so they don't need to be synchronized among several bridge instances; 
  * bridge contracts was segregated into [the separate project](https://github.com/poanetwork/poa-bridge-contracts) and their deployment
    is independent from the Rust side of the bridge. Now bridge contracts:
    * ERC20 is separated from the brdige contracts
    * are upgradable and you don't need to re-configure bridge instances and DApps to use new version of contracts
    * set of validators can be changed without necessity to re-deploy bridge contracts 

### How to build 

Requires `rust` and `cargo`: [installation instructions.](https://www.rust-lang.org/en-US/install.html)

Requires `solc` to be in `$PATH`: [installation instructions.](https://solidity.readthedocs.io/en/develop/installing-solidity.html)

Assuming you've cloned the bridge (`git clone git@github.com:poanetwork/poa-bridge.git`), run

```
cd poa-bridge
make
```

and install `../target/release/bridge` in your `$PATH`.

### Running

```
bridge --config config.toml --database db.toml
```

- `--config` - location of the configuration file. configuration file must exist
- `--database` - location of the database file.

Bridge forces TLS for RPC connections by default. However, in some limited scenarios (like local testing),
this might be undesirable. In this case, you can use `--allow-insecure-rpc-endpoints` option to allow non-TLS
endpoints to be used. Ensure, however, that this option is not going to be used in production.


#### Exit Status Codes

| Code | Meaning              |
|------|----------------------|
|    0 | Success              |
|    1 | Unknwon error        |
|    2 | I/O error            |
|    3 | Shutdown requested   |
|    4 | Insufficient funds   |
|    5 | Gas too low          |
|    6 | Gas price is too low |
|    7 | Nonce reused         |
|   10 | Cannot connect       |
|   11 | Connection lost      |
|   12 | Bridge crashed       |
|   20 | RPC error            |

### Configuration [file example](./examples/config.toml)

```toml
keystore = "/path/to/keystore"

[home]
account = "0x006e27b6a72e1f34c626762f3c4761547aff1421"
password = "home_password.txt"
rpc_host = "http://localhost"
rpc_port = 8545
required_confirmations = 0
poll_interval = 5
request_timeout = 60
default_gas_price = 1_000_000_000 # 1 GWEI

[foreign]
account = "0x006e27b6a72e1f34c626762f3c4761547aff1421"
password = "foreign_password.txt"
rpc_host = "http://localhost"
rpc_port = 9545
required_confirmations = 8
poll_interval = 15
request_timeout = 60
gas_price_oracle_url = "https://gasprice.poa.network"
gas_price_speed = "instant"
gas_price_timeout = 10
default_gas_price = 10_000_000_000 # 10 GWEI

[authorities]

[transactions]
deposit_relay = { gas = 300000 }
withdraw_relay = { gas = 300000 }
withdraw_confirm = { gas = 300000 }
```

#### Options

- `keystore` - path to a keystore directory with JSON keys  

#### home/foreign options

- `home/foreign.account` - authority address on the home (**required**)
- `home/foreign.password` - path to the file containing a password for the validator's account (to decrypt the key from the keystore)
- `home/foreign.rpc_host` - RPC host (**required**)
- `home/foreign.rpc_port` - RPC port (**defaults to 8545**)
- `home/foreign.required_confirmations` - number of confirmation required to consider transaction final on home (default: **12**)
- `home/foreign.poll_interval` - specify how often home node should be polled for changes (in seconds, default: **1**)
- `home/foreign.request_timeout` - specify request timeout (in seconds, default: **3600**)
- `home/foreign.gas_price_oracle_url` - the URL used to query the current gas-price for the home and foreign nodes, this service is known as the gas-price Oracle. This config option defaults to `None` if not supplied in the User's config TOML file. If this config value is `None`, no Oracle gas-price querying will occur, resulting in the config value for `home/foreign.default_gas_price` being used for all gas-prices.
- `home/foreign.gas_price_timeout` - the number of seconds to wait for an HTTP response from the gas price oracle before using the default gas price. Defaults to `10 seconds`.
- `home/foreign.gas_price_speed` - retrieve the gas-price corresponding to this speed when querying from an Oracle. Defaults to `fast`. The available values are: "instant", "fast", "standard", and "slow".
- `home/foreign.default_gas_price` - the default gas price (in WEI) used in transactions with the home or foreign nodes. The `default_gas_price` is used when the Oracle cannot be reached. The default value is `15_000_000_000` WEI (ie. 15 GWEI).
- `home/foreign.concurrent_http_requests` - the number of concurrent HTTP requests allowed in-flight (default: **64**)

#### transaction options

- `transaction.deposit_relay.gas` - specify how much gas should be consumed by deposit relay
- `transaction.withdraw_confirm.gas` - specify how much gas should be consumed by withdraw confirm
- `transaction.withdraw_relay.gas` - specify how much gas should be consumed by withdraw relay

### Database file format

```toml
home_contract_address = "0x49edf201c1e139282643d5e7c6fb0c7219ad1db7"
foreign_contract_address = "0x49edf201c1e139282643d5e7c6fb0c7219ad1db8"
checked_deposit_relay = 120
checked_withdraw_relay = 121
checked_withdraw_confirm = 121
```

**all fields are required**

- `home_contract_address` - address of the bridge contract on home chain
- `foreign_contract_address` - address of the bridge contract on foreign chain
- `checked_deposit_relay` - number of the last block for which an authority has relayed deposits to the foreign
- `checked_withdraw_relay` - number of the last block for which an authority has relayed withdraws to the home
- `checked_withdraw_confirm` - number of the last block for which an authority has confirmed withdraw
