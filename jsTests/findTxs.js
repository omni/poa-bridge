let utils = require("./utils");

for (let i = 14000; i < 15975; i++) {
	console.log(i);
	utils.getBlockData("right", i, function(res) {
		if (!res) return
		for (let j = 0; j < res.transactions.length; j++) {
			let txHash = res.transactions[j]
			console.log("block number = ",i, "tx hash = ",txHash)
		}
	})
}
