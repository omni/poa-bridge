let utils = require("./utils");
let investorAddr = "0x93C70B45d0e86e99A84dF0966Ac59DC71540c8d1"

//immediately

/*for (var i = 0; i < 200; i++) {
	let tokens = 1;
	utils.buyFromWizard(investorAddr, tokens);
}*/


let i = 0;
let timer = setInterval(function() {
	i++;
	console.log(i);
	if (i >= 500) clearInterval(timer)
	buy();
}, 1000)

function buy () {
	//let tokens = Math.floor(Math.random() * 10) + 1;
	let tokens = 1;
	utils.buyFromWizard(investorAddr, tokens);
}


//let _from = "0x00dB9af45C6f241432F2cBE412c6969cB7778d98"
//let _to = "0xeaeba7869e23a328a0a92620bba1a7a6aaed26cb"
//utils.sendTX("left", _from, _to);
