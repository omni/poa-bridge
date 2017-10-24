let utils = require("./utils");

for (let i = 1; i < 50; i++) {
	utils.getBlockData("left", i, function(res) {
		for (let j = 0; j < res.transactions.length; j++) {
			let txHash = res.transactions[j]
			utils.getTxReceipt("left", txHash, function(txRes) {
				if (txRes.contractAddress) console.log("block number = ",i, "contract address = ",txRes.contractAddress);
			});
		}
	})
}
