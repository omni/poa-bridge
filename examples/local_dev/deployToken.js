var Web3 = require('web3');

var web3 = new Web3();
web3.setProvider(new web3.providers.HttpProvider('http://127.0.0.1:8551'));  // 8551 for test foreign parity rpcc port

var contract = new web3.eth.Contract([{"constant":true,"inputs":[],"name":"mintingFinished","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_spender","type":"address"},{"name":"_value","type":"uint256"}],"name":"approve","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[],"name":"totalSupply","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_from","type":"address"},{"name":"_to","type":"address"},{"name":"_value","type":"uint256"}],"name":"transferFrom","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"receiver","type":"address"},{"name":"amount","type":"uint256"}],"name":"mint","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"","type":"address"}],"name":"mintAgents","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"addr","type":"address"},{"name":"state","type":"bool"}],"name":"setMintAgent","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"_owner","type":"address"}],"name":"balanceOf","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"owner","outputs":[{"name":"","type":"address"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"_to","type":"address"},{"name":"_value","type":"uint256"}],"name":"transfer","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"spender","type":"address"},{"name":"tokens","type":"uint256"},{"name":"data","type":"bytes"}],"name":"approveAndCall","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"_owner","type":"address"},{"name":"_spender","type":"address"}],"name":"allowance","outputs":[{"name":"remaining","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"isToken","outputs":[{"name":"weAre","type":"bool"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"newOwner","type":"address"}],"name":"transferOwnership","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"inputs":[],"payable":false,"stateMutability":"nonpayable","type":"constructor"},{"anonymous":false,"inputs":[{"indexed":false,"name":"addr","type":"address"},{"indexed":false,"name":"state","type":"bool"}],"name":"MintingAgentChanged","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"previousOwner","type":"address"},{"indexed":true,"name":"newOwner","type":"address"}],"name":"OwnershipTransferred","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"receiver","type":"address"},{"indexed":false,"name":"amount","type":"uint256"}],"name":"Minted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"owner","type":"address"},{"indexed":true,"name":"spender","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Approval","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"from","type":"address"},{"indexed":true,"name":"to","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Transfer","type":"event"}]);
var code = '0x60606040526003805460a060020a60ff0219169055341561001f57600080fd5b60038054600160a060020a03191633600160a060020a0316179055610a00806100496000396000f3006060604052600436106100cf5763ffffffff7c010000000000000000000000000000000000000000000000000000000060003504166305d2035b81146100d4578063095ea7b3146100fb57806318160ddd1461011d57806323b872dd1461014257806340c10f191461016a57806342c1867b1461018e57806343214675146101ad57806370a08231146101d15780638da5cb5b146101f0578063a9059cbb1461021f578063cae9ca5114610241578063dd62ed3e146102a6578063eefa597b146102cb578063f2fde38b146102de575b600080fd5b34156100df57600080fd5b6100e76102fd565b604051901515815260200160405180910390f35b341561010657600080fd5b6100e7600160a060020a036004351660243561031e565b341561012857600080fd5b6101306103c6565b60405190815260200160405180910390f35b341561014d57600080fd5b6100e7600160a060020a03600435811690602435166044356103cc565b341561017557600080fd5b61018c600160a060020a03600435166024356104cd565b005b341561019957600080fd5b6100e7600160a060020a03600435166105b1565b34156101b857600080fd5b61018c600160a060020a036004351660243515156105c6565b34156101dc57600080fd5b610130600160a060020a036004351661067a565b34156101fb57600080fd5b610203610695565b604051600160a060020a03909116815260200160405180910390f35b341561022a57600080fd5b6100e7600160a060020a03600435166024356106a4565b341561024c57600080fd5b6100e760048035600160a060020a03169060248035919060649060443590810190830135806020601f8201819004810201604051908101604052818152929190602084018383808284375094965061075795505050505050565b34156102b157600080fd5b610130600160a060020a03600435811690602435166108d3565b34156102d657600080fd5b6100e76108fe565b34156102e957600080fd5b61018c600160a060020a0360043516610903565b60035474010000000000000000000000000000000000000000900460ff1681565b600081158015906103535750600160a060020a0333811660009081526002602090815260408083209387168352929052205415155b1561035d57600080fd5b600160a060020a03338116600081815260026020908152604080832094881680845294909152908190208590557f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b9259085905190815260200160405180910390a350600192915050565b60005481565b600160a060020a03808416600090815260026020908152604080832033851684528252808320549386168352600190915281205490919061040d908461099e565b600160a060020a03808616600090815260016020526040808220939093559087168152205461043c90846109c2565b600160a060020a03861660009081526001602052604090205561045f81846109c2565b600160a060020a03808716600081815260026020908152604080832033861684529091529081902093909355908616917fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9086905190815260200160405180910390a3506001949350505050565b600160a060020a03331660009081526004602052604081205460ff1615156104f457600080fd5b60035474010000000000000000000000000000000000000000900460ff161561051c57600080fd5b81670de0b6b3a76400000290506105356000548261099e565b6000908155600160a060020a03841681526001602052604090205461055a908261099e565b600160a060020a0384166000818152600160205260408082209390935590917fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9084905190815260200160405180910390a3505050565b60046020526000908152604090205460ff1681565b60035433600160a060020a039081169116146105e157600080fd5b60035474010000000000000000000000000000000000000000900460ff161561060957600080fd5b600160a060020a03821660009081526004602052604090819020805460ff19168315151790557f4b0adf6c802794c7dde28a08a4e07131abcff3bf9603cd71f14f90bec7865efa908390839051600160a060020a039092168252151560208201526040908101905180910390a15050565b600160a060020a031660009081526001602052604090205490565b600354600160a060020a031681565b600160a060020a0333166000908152600160205260408120546106c790836109c2565b600160a060020a0333811660009081526001602052604080822093909355908516815220546106f6908361099e565b600160a060020a0380851660008181526001602052604090819020939093559133909116907fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef9085905190815260200160405180910390a350600192915050565b600160a060020a03338116600081815260026020908152604080832094881680845294909152808220869055909291907f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b9259086905190815260200160405180910390a383600160a060020a0316638f4ffcb1338530866040518563ffffffff167c01000000000000000000000000000000000000000000000000000000000281526004018085600160a060020a0316600160a060020a0316815260200184815260200183600160a060020a0316600160a060020a0316815260200180602001828103825283818151815260200191508051906020019080838360005b8381101561086b578082015183820152602001610853565b50505050905090810190601f1680156108985780820380516001836020036101000a031916815260200191505b5095505050505050600060405180830381600087803b15156108b957600080fd5b5af115156108c657600080fd5b5060019695505050505050565b600160a060020a03918216600090815260026020908152604080832093909416825291909152205490565b600190565b60035433600160a060020a0390811691161461091e57600080fd5b600160a060020a038116151561093357600080fd5b600354600160a060020a0380831691167f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e060405160405180910390a36003805473ffffffffffffffffffffffffffffffffffffffff1916600160a060020a0392909216919091179055565b60008282018381108015906109b35750828110155b15156109bb57fe5b9392505050565b6000828211156109ce57fe5b509003905600a165627a7a72305820115a5cbcb52069632d11d73b514884ef0840abdea5772e141d342b3f3dfd95d10029';

console.log("Deploying contract....");

contract.deploy({
    data: code
}).send ({
    from: '0x00bd138abd70e2f00903268f3db08f2d25677c9e',
    gas: 0
}, function(error, transactionHash) {})
.on('error', function(error) {
    console.log("Error: ", error);
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
.then(function(newContractInstance) {
    console.log("newContractInstance address: ", newContractInstance.options.address)
});