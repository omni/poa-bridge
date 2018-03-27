var Web3 = require('web3');

var web3 = new Web3();
web3.setProvider(new web3.providers.HttpProvider('http://127.0.0.1:8551'));  // 8551 for test foreign parity rpc port

var tokenAddress = 'eBD3944aF37ccc6b67ff61239AC4feF229c8f69f'; // no 0x
var code = '0x4321467500000000000000000000000000bd138abd70e2f00903268f3db08f2d25677c9e0000000000000000000000000000000000000000000000000000000000000001';

console.log("Sending transaction....");

web3.eth.sendTransaction({
    from: '0x00bd138abd70e2f00903268f3db08f2d25677c9e',
    to: '0x' + tokenAddress,
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
