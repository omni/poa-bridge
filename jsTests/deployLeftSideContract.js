let fs = require('fs');
let utils = require("./utils");

let config = JSON.parse(fs.readFileSync('./config.json', 'utf8'));

let Web3 = require('web3');
let web3;
if (typeof web3 !== 'undefined') {
  web3 = new Web3(web3.currentProvider);
} else {
  web3 = new Web3(new Web3.providers.HttpProvider(config.rpc));
}

let contractABI = config.contract.left.abi;
let compiled = config.contract.left.bin;

deployContract();

function deployContract() {
	let from = "0x00a6F6A140a3E2991c555dCCb36B4FBa7A46b0D8";
	let estimatedGas = 2000000;
	let gasPrice = 21000000000;
	let params = [
		1,
		[from]
	]
	

	web3.eth.defaultAccount = from;

	let contractInstance = new web3.eth.Contract(contractABI);

	let deployOpts = {
      data: compiled,
      arguments: params
    };

    let sendOpts = {
      from: from,
      gas: estimatedGas,
      gasPrice: gasPrice
    };

    let isMined = false;

    contractInstance.deploy(deployOpts).send(sendOpts)
    .on('error', function(error) { 
      console.log(error);
      return; 
    })
    .on('transactionHash', function(transactionHash){ 
      console.log("contract deployment transaction: " + transactionHash);

      utils.checkTxMined(web3, transactionHash, function txMinedCallback(receipt) {
        if (isMined) return;

        if (receipt) {
          if (receipt.blockNumber) {
            console.log("Contract deployment is mined from polling of tx receipt");
            isMined = true;
            console.log(receipt.contractAddress) // instance with the new contract address
            return;
          } else {
            console.log("Still mining... Polling of transaction once more");
            setTimeout(function() {
              utils.checkTxMined(web3, transactionHash, txMinedCallback)
            }, 5000);
          }
        } else {
          console.log("Still mining... Polling of transaction once more");
          setTimeout(function() {
            utils.checkTxMined(web3, transactionHash, txMinedCallback)
          }, 5000);
        }
      })
    })
};