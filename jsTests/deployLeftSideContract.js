let fs = require('fs');
let utils = require("./utils");

let config = JSON.parse(fs.readFileSync('./config.json', 'utf8'));

let Web3 = require('web3');
let web3;
if (typeof web3 !== 'undefined') {
  web3 = new Web3(web3.currentProvider);
} else {
  web3 = new Web3(new Web3.providers.HttpProvider(config.rpc.left));
}

var homeBridgeBin = "0x" + fs.readFileSync('../contracts/HomeBridge.bin').toString('utf8');
var homeBridgeABI = JSON.parse(fs.readFileSync('../contracts/HomeBridge.abi').toString('utf8'));

let contractABI = homeBridgeABI;
let compiled = homeBridgeBin;

deployContract();

function deployContract() {
	let from = "0xeaeba7869e23a328a0a92620bba1a7a6aaed26cb";
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