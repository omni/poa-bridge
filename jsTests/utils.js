let fs = require("fs");
let Web3 = require("web3");

function attachToContract(side, cb) {
	var config = getConfig();
	configureWeb3(function(err, web3, config, defaultAccount) {
		if (err) return console.log(err);

		var contractABI = config.contract[side].abi;
		var contractAddress = config.contract[side].addr;

		if(!web3.isConnected()) {
			if (cb) cb({code: 200, title: "Error", message: "check RPC"}, null);
		} else {
			web3.eth.defaultAccount = defaultAccount;
			
			var MyContract = web3.eth.contract(contractABI);

			contract = MyContract.at(contractAddress);
			
			if (cb) cb(null, contract, config, web3);
		}
	});
}

function getConfig() {
	var config = JSON.parse(fs.readFileSync('./config.json', 'utf8'));
	return config;
}

function configureWeb3(cb) {
	var config = getConfig();
	var web3;
	if (typeof web3 !== 'undefined') {
	  web3 = new Web3(web3.currentProvider);
	} else {
	  web3 = new Web3(new Web3.providers.HttpProvider(config.rpc));
	}
	if(!web3.isConnected()) {
		var err = '{code: 500, title: "Error", message: "check RPC"}';
		cb(err, web3, config);
	} else {
		//console.log(web3.eth.accounts);
		
		web3.eth.defaultAccount = config.account;
		var defaultAccount = web3.eth.defaultAccount;
		//console.log("web3.eth.defaultAccount:");
		//console.log(web3.eth.defaultAccount);
		cb(null, web3, config, defaultAccount);
	}
}

function getTxReceipt(txhash) {
	console.log("***getTxReceipt***");
	configureWeb3(function(err, web3, config, defaultAccount) {
		web3.eth.getTransactionReceipt(txhash, function(err, val) {
			console.log(val);
		});
	});
}

function getTxData(txhash) {
	console.log("***getTxData***");
	configureWeb3(function(err, web3, config, defaultAccount) {
		web3.eth.getTransaction(txhash, function(err, val) {
			console.log(val);
		});
	});
}

function getBalance(addr) {
	var config = getConfig();
	configureWeb3(function(err, web3, config, defaultAccount) {
		let balance = web3.eth.getBalance(addr)
		console.log("balance: " + balance)
	});
}

module.exports = {
	attachToContract: attachToContract,
	getConfig: getConfig,
	configureWeb3: configureWeb3,
	getTxReceipt: getTxReceipt,
	getTxData: getTxData,
	getBalance: getBalance
}
