let fs = require("fs")
let Web3 = require("web3")
let toml = require("toml")

function attachToContract(side, cb, _ABI, _addr) {
	configureWeb3(side, function(err, web3, config, defaultAccount) {
	    web3.eth.defaultAccount = config.account;

	    var abi;
		var addr;


		if (!_ABI && !_addr) {
			var dbtoml = fs.readFileSync('../examples/db.toml').toString('utf8');
		    var db = toml.parse(dbtoml);

		    //var homeBridgeBin = "0x" + fs.readFileSync('../contracts/HomeBridge.bin').toString('utf8');
		    var homeBridgeABI = JSON.parse(fs.readFileSync('../contracts/HomeBridge.abi').toString('utf8'));

		    //var foreignBridgeBin = "0x" + fs.readFileSync('../contracts/ForeignBridge.bin').toString('utf8');
		    var foreignBridgeABI = JSON.parse(fs.readFileSync('../contracts/ForeignBridge.abi').toString('utf8'));

		    if (side == "left") {
		    	abi = homeBridgeABI;
				addr = db.home_contract_address;
		    } else if (side == "right") {
		    	abi = foreignBridgeABI;
				addr = db.foreign_contract_address;
		    }
		} else {
			abi = _ABI
			addr = _addr
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

function getTxReceipt(side, txhash, cb) {
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		web3.eth.getTransactionReceipt(txhash, function(err, val) {
			if (!cb) console.log(val);
			if (cb) cb(val);
		});
	});
}

function getTxData(side, txhash) {
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

function getAuthorities(side) {
	attachToContract(side, function(err, contract, web3) {
		contract.methods.authorities(0).call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getAuthorities from " + side + ":");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getBlockData(side, blokNumber, cb) {
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		web3.eth.getBlock(blokNumber).then(
			function(res) {
				if (!cb) console.log(res)
				if (cb) cb(res)
			}
		);
	});
}

function getRequiredSignatures(side) {
	attachToContract(side, function(err, contract, web3) {
		contract.methods.requiredSignatures().call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getRequiredSignatures from " + side + ":");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getTokenBalanceOf(addr) {
	attachToContract("right", function(err, contract, web3) {
		contract.methods.balances(addr).call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getTokenBalanceOf from right:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getERC20TokenBalanceOf(addr, _ABI, _addr) {
	attachToContract("right", function(err, contract, web3) {
		contract.methods.balanceOf(addr).call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getTokenBalance of " + addr + " from the right side:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	}, _ABI, _addr);
}

function buyFromWizard(addr) {
	attachToContract("left", function(err, contract, web3) {
		contract.methods.buy().send({from: web3.eth.defaultAccount, value: 1000000000000000, from: addr}).then(function(err, result) {
			console.log("buy from left:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function buyFromBridge(addr) {
	attachToContract("left", function(err, contract, web3) {
		web3.eth.sendTransaction({from: web3.eth.defaultAccount, value: 1000000000000000, from: addr, to: db.home_contract_address}).then(function(err, result) {
			console.log("buy from left:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function sendTX(side, from, to) {
	configureWeb3(side, function(err, web3, config, defaultAccount) {
		web3.eth.sendTransaction({from: from, value: 1000000000000000, to: to}).then(function(err, result) {
			console.log("sendTX:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getDeposits(hash) {
	attachToContract("right", function(err, contract, config, web3) {
		contract.methods.deposits(hash).call({from: web3.eth.defaultAccount}, function(result) {
			console.log("getBalances from right:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

module.exports = {
	attachToContract,
	getConfig,
	configureWeb3,
	getTxReceipt,
	getTxData,
	getBalance,
	checkTxMined,
	getAuthorities,
	getBlockData,
	getRequiredSignatures,
	getTokenBalanceOf,
	getERC20TokenBalanceOf,
	buyFromWizard,
	buyFromBridge,
	getDeposits,
	sendTX
}
