let utils = require("./utils");

for (let i = 30500; i < 33266; i++) {
	utils.getBlockData("left", i, function(res) {
		if (!res) return
		for (let j = 0; j < res.transactions.length; j++) {
			let txHash = res.transactions[j]
			if (!txHash) return
			utils.getTxReceipt("left", txHash, function(txRes) {
				if (txRes.contractAddress) console.log("block number = ",i, "contract address = ",txRes.contractAddress);
			});
		}
	})
}
