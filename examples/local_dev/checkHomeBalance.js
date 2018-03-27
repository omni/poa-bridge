var Web3 = require('web3');

var web3 = new Web3();
web3.setProvider(new web3.providers.HttpProvider('http://127.0.0.1:8550'));  // 8550 for test home parity rpc port

var userAddress = '0x00a329c0648769a73afac7f9381e08fb43dbea72';

console.log("Checking balance....");

web3.eth.getBalance(userAddress)
 .then( console.log);
