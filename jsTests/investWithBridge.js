let utils = require("./utils");

let investorAddr = "0xDd0BB0e2a1594240fED0c2f2c17C1E9AB4F87126"

sendMoneyToContract(investorAddr);

function sendMoneyToContract(addr) {
	utils.attachToContract("left", function(err, contract, config, web3) {
		if(!web3.isConnected()) {
			cb({code: 500, title: "Error", message: "check RPC"}, null);
		} else {
			console.log(config.contract.left.addr);
			web3.eth.sendTransaction({
				gas: 800000,
				from: addr,
				to: config.contract.left.addr,
				value: web3.toWei(0.01, "ether")
			}, function(err, result) {
				if (err) console.log(err);
				console.log("sendMoneyToContract:");
				console.log("result: " + result);
			})
		}
	});
}