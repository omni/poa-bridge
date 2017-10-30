let utils = require("./utils");
let investorAddr = "0xDd0BB0e2a1594240fED0c2f2c17C1E9AB4F87126"


for (var i = 0; i < 100; i++) {
	utils.buyFromWizard(investorAddr);
}


//let _from = "0x00dB9af45C6f241432F2cBE412c6969cB7778d98"
//let _to = "0xeaeba7869e23a328a0a92620bba1a7a6aaed26cb"
//utils.sendTX("left", _from, _to);
