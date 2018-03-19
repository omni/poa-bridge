let utils = require("./utils");
let investorAddr = "0x00dB9af45C6f241432F2cBE412c6969cB7778d98"

//immediately

/*for (var i = 0; i < 200; i++) {
	let tokens = 1;
	utils.buyFromWizard(investorAddr, tokens);
}*/


let i = 0;
let timer = setInterval(function() {
	i++;
	console.log(i);
	if (i >= 11000) clearInterval(timer)
	buy();
}, 500)

function buy () {
	let tokens = Math.floor(Math.random() * 100) + 1;
	//let tokens = 1;
	utils.buyFromWizard(investorAddr, tokens);
}


//let _from = "0x00dB9af45C6f241432F2cBE412c6969cB7778d98"
//let _to = "0xeaeba7869e23a328a0a92620bba1a7a6aaed26cb"
//utils.sendTX("left", _from, _to);
