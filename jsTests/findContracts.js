let utils = require("./utils");

for (let i = 16000; i < 17000; i++) {
	utils.getBlockData("right", i, function(res) {
		for (let j = 0; j < res.transactions.length; j++) {
			let txHash = res.transactions[j]
			if (!txHash) return
			utils.getTxReceipt("right", txHash, function(txRes) {
				if (txRes.contractAddress) console.log("block number = ",i, "contract address = ",txRes.contractAddress);
			});
		}
	})
}
