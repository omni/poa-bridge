let fs = require("fs")
let Web3 = require("web3")
let toml = require("toml")

function attachToContract(side, cb) {
	configureWeb3(side, function(err, web3, config, defaultAccount) {
	    web3.eth.defaultAccount = config.account;

	    var dbtoml = fs.readFileSync('../examples/db.toml').toString('utf8');
	    var db = toml.parse(dbtoml);

	    var homeBridgeBin = "0x" + fs.readFileSync('../contracts/HomeBridge.bin').toString('utf8');
	    var homeBridgeABI = JSON.parse(fs.readFileSync('../contracts/HomeBridge.abi').toString('utf8'));

	    var foreignBridgeBin = "0x" + fs.readFileSync('../contracts/ForeignBridge.bin').toString('utf8');
	    var foreignBridgeABI = JSON.parse(fs.readFileSync('../contracts/ForeignBridge.abi').toString('utf8'));

	    var abi;
		var addr;
	    if (side == "left") {
	    	abi = homeBridgeABI;
			addr = db.home_contract_address;
	    } else if (side == "right") {
	    	abi = foreignBridgeABI;
			addr = db.foreign_contract_address;
	    }

    	//var abi = config.contract[side].abi;
		//var addr = config.contract[side].addr;
		console.log("web3.eth.defaultAccount:" + web3.eth.defaultAccount);
		
		let contractInstance = new web3.eth.Contract(abi, addr, {
	      from: web3.eth.defaultAccount
	    });
		
		if (cb) cb(null, contractInstance, web3);
	});
}

function getConfig() {
	var config = JSON.parse(fs.readFileSync('./config.json', 'utf8'));
	return config;
}

function configureWeb3(side, cb) {
	var config = getConfig();
	var web3;
	if (typeof web3 !== 'undefined') {
	  web3 = new Web3(web3.currentProvider);
	} else {
	  web3 = new Web3(new Web3.providers.HttpProvider(config.rpc[side]));
	}
	//console.log(web3.eth.accounts);
	
	//web3.eth.defaultAccount = config.account;
	var defaultAccount = web3.eth.defaultAccount;
	//console.log("web3.eth.defaultAccount:");
	//console.log(web3.eth.defaultAccount);
	cb(null, web3, config, defaultAccount);
}

function getTxReceipt(side, txhash) {
	console.log("***getTxReceipt***");
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		web3.eth.getTransactionReceipt(txhash, function(err, val) {
			console.log(val);
		});
	});
}

function getTxData(side, txhash) {
	console.log("***getTxData***");
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		web3.eth.getTransaction(txhash, function(err, val) {
			console.log(val);
		});
	});
}

function getBalance(side, addr) {
	var config = getConfig();
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		let balance = web3.eth.getBalance(addr).then(function(balance) {
			console.log("balance: " + balance)
		})
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
