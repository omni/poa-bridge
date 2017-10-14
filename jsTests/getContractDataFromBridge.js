let utils = require("./utils");

let investorAddr = "0xDd0BB0e2a1594240fED0c2f2c17C1E9AB4F87126"
let txHash = "0xc7f77a849a89bf8f7d18fd56d00a99c5463f3d15851075e2573a9dc98f92e493"

//check that contract is created correctly: should return values
getAuthorities("left");
getRequiredSignatures("left");
//getAuthorities("right");
//getRequiredSignatures("right");

utils.getTxReceipt(txHash);
utils.getTxData(txHash);

let config = utils.getConfig();
//getTokenBalanceOf(investorAddr);
//utils.getBalance(config.contract.left.addr);

function getAuthorities(side) {
	utils.attachToContract(side, function(err, contract, web3) {
		contract.methods.authorities(0).call({from: web3.eth.defaultAccount}, function(err, result) {
			console.log("getAuthorities:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getRequiredSignatures(side) {
	utils.attachToContract(side, function(err, contract, web3) {
		contract.methods.requiredSignatures().call({from: web3.eth.defaultAccount}, function(err, result) {
			console.log("getAuthorities:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getTokenBalanceOf(addr) {
	utils.attachToContract("right", function(err, contract, web3) {
		contract.methods.balances(addr).call({from: web3.eth.defaultAccount}, function(err, result) {
			console.log("getBalances:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getDeposits(hash) {
	utils.attachToContract("right", function(err, contract, config, web3) {
		contract.methods.deposits(hash).call({from: web3.eth.defaultAccount}, function(err, result) {
			console.log("getBalances:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}
