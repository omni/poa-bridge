let utils = require("./utils");
let investorAddr = "0xDd0BB0e2a1594240fED0c2f2c17C1E9AB4F87126"

//immediately
/*
for (var i = 0; i < 1000; i++) {
	utils.buyFromWizard(investorAddr);
}
*/

let i = 0;
let timer = setInterval(function() {
	i++;
	console.log(i);
	if (i >= 100) clearInterval(timer)
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
