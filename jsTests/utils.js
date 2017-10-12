let fs = require("fs");
let Web3 = require("web3");

function attachToContract(side, cb) {
	configureWeb3(function(err, web3, config, defaultAccount) {
	  web3.eth.getAccounts().then((accounts) => {
	    web3.eth.defaultAccount = accounts[0];
	    	var abi = config.contract[side].abi;
			var addr = config.contract[side].addr;
			console.log("web3.eth.defaultAccount:" + web3.eth.defaultAccount);
			
			let contractInstance = new web3.eth.Contract(abi, addr, {
		      from: web3.eth.defaultAccount
		    });
			
			if (cb) cb(null, contractInstance, web3);
	  });
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
	//console.log(web3.eth.accounts);
	
	web3.eth.defaultAccount = config.account;
	var defaultAccount = web3.eth.defaultAccount;
	//console.log("web3.eth.defaultAccount:");
	//console.log(web3.eth.defaultAccount);
	cb(null, web3, config, defaultAccount);
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

function checkTxMined(web3, txhash, cb) {
  web3.eth.getTransactionReceipt(txhash, function(err, receipt) {
    if (receipt)
      console.log(receipt);
    cb(receipt);
  });
}

module.exports = {
	attachToContract: attachToContract,
	getConfig: getConfig,
	configureWeb3: configureWeb3,
	getTxReceipt: getTxReceipt,
	getTxData: getTxData,
	getBalance: getBalance,
	checkTxMined: checkTxMined
}
