let utils = require("./utils");

let investorAddr = "0xDd0BB0e2a1594240fED0c2f2c17C1E9AB4F87126"
let txHash = "0xf1488e49928916f7c34d231aa64fbc64f4bb79f45b3197330a514b8051746367"

//check that contract is created correctly: should return values
getAuthorities("left");
getRequiredSignatures("left");
getAuthorities("right");
getRequiredSignatures("right");

utils.getTxReceipt(txHash);
utils.getTxData(txHash);

let config = utils.getConfig();
getTokenBalanceOf(investorAddr);
utils.getBalance(config.contract.left.addr);

function getAuthorities(side) {
	utils.attachToContract(side, function(err, contract, config, web3) {
		if(!web3.isConnected()) {
			cb({code: 500, title: "Error", message: "check RPC"}, null);
		} else {
			contract.authorities.call(0, {from: web3.eth.defaultAccount}, function(err, result) {
				console.log("getAuthorities:");
				if (err) console.log(err);
				console.log("result: " + result);
			});
		}
	});
}

function getRequiredSignatures(side) {
	utils.attachToContract(side, function(err, contract, config, web3) {
		if(!web3.isConnected()) {
			cb({code: 500, title: "Error", message: "check RPC"}, null);
		} else {
			contract.requiredSignatures.call({from: web3.eth.defaultAccount}, function(err, result) {
				console.log("getAuthorities:");
				if (err) console.log(err);
				console.log("result: " + result);
			});
		}
	});
}

function getTokenBalanceOf(addr) {
	utils.attachToContract("right", function(err, contract, config, web3) {
		if(!web3.isConnected()) {
			cb({code: 500, title: "Error", message: "check RPC"}, null);
		} else {
			contract.balances.call(addr, {from: web3.eth.defaultAccount}, function(err, result) {
				console.log("getBalances:");
				if (err) console.log(err);
				console.log("result: " + result);
			});
		}
	});
}

function getDeposits(hash) {
	utils.attachToContract("right", function(err, contract, config, web3) {
		if(!web3.isConnected()) {
			cb({code: 500, title: "Error", message: "check RPC"}, null);
		} else {
			contract.deposits.call(hash, {from: web3.eth.defaultAccount}, function(err, result) {
				console.log("getBalances:");
				if (err) console.log(err);
				console.log("result: " + result);
			});
		}
	});
}
