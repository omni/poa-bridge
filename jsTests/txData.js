let utils = require("./utils");

let txHash = "0x2a7d607300016547e4be03b06a4b354d0ff9209af51b0933a0bfa9a3fac61a1b"

utils.getTxReceipt("right", txHash);
utils.getTxData("left", txHash);