var Web3 = require('web3');

var web3 = new Web3();
web3.setProvider(new web3.providers.HttpProvider('http://127.0.0.1:8551'));  // 8551 for test foreign parity rpc port

var userAddress = '00a329c0648769A73afAc7F9381E08FB43dBEA72'; // without 0x
//var code = '0x8f4ffcb1000000000000000000000000' + userAddress + '00000000000000000000000000000000000000000000000000000000000186a0000000000000000000000000ebd3944af37ccc6b67ff61239ac4fef229c8f69f00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000000';
var code = '0x8f4ffcb1000000000000000000000000' + userAddress + '00000000000000000000000000000000000000000000000000000000000186a0000000000000000000000000ebd3944af37ccc6b67ff61239ac4fef229c8f69f';
var foreignContractAddress = '0x8886F0F21042e73cc1C7d2c48a3135666492981F';
console.log("Sending transaction....");

web3.eth.sendTransaction({
    from: '0x' + userAddress,
    to: foreignContractAddress,
    data: code
})
.on('transactionHash', function(transactionHash) {
    console.log("TransHash: ", transactionHash);
} )
.on('receipt', function(receipt) {
    console.log("Receipt address: ", receipt.contractAddress);
})
.on('confirmation', function(confirmationNumber, receipt) {
    console.log("Confirmation number: ", confirmationNumber);
    console.log("Confirmation Receipt: ", receipt);
})
.on('error', console.error);
