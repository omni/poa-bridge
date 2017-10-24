let utils = require("./utils");
let fs = require("fs")
let toml = require("toml")

let investorAddr = "0xEaeBA7869E23A328a0A92620BbA1A7a6aaED26cB"
let txHash = "0x325f14869a538f2dd70f8b0f77a3f965db10eeb65a44a27dcb83c16c06196316"

//check that contract is created correctly: should return values
//getAuthorities("left");
//getRequiredSignatures("left");
getAuthorities("right");
getRequiredSignatures("right");

utils.getTxReceipt("left", txHash);
utils.getTxData("left", txHash);

getTokenBalanceOf(investorAddr);

var dbtoml = fs.readFileSync('../examples/db.toml').toString('utf8');
var db = toml.parse(dbtoml);

utils.getBalance("left", db.home_contract_address);

//buyFromWizard(investorAddr);

function getAuthorities(side) {
	utils.attachToContract(side, function(err, contract, web3) {
		contract.methods.authorities(0).call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getAuthorities from " + side + ":");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getRequiredSignatures(side) {
	utils.attachToContract(side, function(err, contract, web3) {
		contract.methods.requiredSignatures().call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getRequiredSignatures from " + side + ":");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getTokenBalanceOf(addr) {
	utils.attachToContract("right", function(err, contract, web3) {
		contract.methods.balances(addr).call({from: web3.eth.defaultAccount}).then(function(result) {
			console.log("getTokenBalanceOf from right:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function buyFromWizard(addr) {
	utils.attachToContract("left", function(err, contract, web3) {
		contract.methods.buy().send({from: web3.eth.defaultAccount, value: 1000000000000000, from: addr}).then(function(err, result) {
			console.log("buy from left:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function buyFromBridge(addr) {
	utils.attachToContract("left", function(err, contract, web3) {
		web3.eth.sendTransaction({from: web3.eth.defaultAccount, value: 1000000000000000, from: addr, to: db.home_contract_address}).then(function(err, result) {
			console.log("buy from left:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}

function getDeposits(hash) {
	utils.attachToContract("right", function(err, contract, config, web3) {
		contract.methods.deposits(hash).call({from: web3.eth.defaultAccount}, function(result) {
			console.log("getBalances from right:");
			if (err) console.log(err);
			console.log("result: " + result);
		});
	});
}
