#﻿Interact with poa parity-bridge
These are notes about how to interact with the poa parity-bridge.  After setting up the bridge the question becomes how do we know it’s working and how do we interact with it for testing.  We can run the unit and integration tests included with the project, and that shows it’s working, but it doesn’t really help us understand how it works.  So, I’ve written some scripts to interact with it to help understand it better.  Please provide comments and suggestions.

I’ve provided a zip file containing scripts and config files for testing.  Unzip this into the parity-bridge/examples folder of your bridge source tree.  It creates a folder called local_dev that contains scripts and config files.  We’ll run these from within the local_dev folder so references to files inside the project will work.  The parity node instances we’ll be starting will create local dev chains, the data for these will be stored in local_dev/tmp, so if you want to start fresh you can delete that folder and start over, or keep the folder so that you don’t need to start from the beginning every time.

We’ll be working in four terminal sessions, so that we can see what’s happening at each stage as we progress:
- Parity home node, connected to home dev chain.
- Parity foreign node, conneted to foreign dev chain.
- Bridge, an instance of the parity-bridge we are testing.
- One for interacting with these other instances, this is were we’ll be running scripts.

We’ll enable the parity UI on the foreign bridge so that we can see things that happen to the contracts. 

This is basically following the code in integration-tests\tests\basic_deposit_then_withdraw.rs and breaking it into parts that we can test individually.

Open a terminal, and working in the local_dev folder 
`cd examples/local_dev`

General setup, there are a few node js scipts that use web3 to interact with the nodes, so you’ll need nodeje and npm installed.  Then install the script dependencies.
`npm install`

##Create accounts
First we’ll create accounts with test ether for us to work with, since the result of creating these accounts is stored in the tmp folder you only need to do this once. We can do this from any of the terminals, since we won’t keep these running.

* In a terminal window, start the home parity node
`./parity_start_home.sh`
* In another terminal, run the script to Create authority account on home chain 
`./curl_create_acc_home.sh`
You’ll should see a result indicating that account 0x00bd138… was created.
* Stop parity (ctrl-c) in the terminal that you started the parity node in. 
* Start the foreign parity node
`./paity_start_foreign.sh`
* Create authority account on foreign
 `./curl_create_acc_foreign.sh`
Again you should see the same account address created, but this is on the foreign bridge.
 * Stop parity node

## Deploy test token to foreign chain
These are the token that will be used on the foreign chain, we’ll deploy an token contract, then connect that contract to the foreign bridge contract.
 
 * In terminal, start foreign node unlocked
 `./parity_start_foreign_unlocked.sh`
 * run deployment script, this is a node js script that deploys the token contract into the foreign chain.  Note the abi and bin is from compiled_contracts/Token.abi, I hard coded them into the deployToken.js script for simplicity, since this is for learning.
 `node deployToken.js`
 The script will run for a little while as it’s monitoring events for this deployment, after the first confirmation you can stop it (or let it run).  You’ll need the contractAddress that was created as a result of deploying these tokens.

Next we'll setup the token contract to watch, open parity ui by browsing to http://127.0.0.1:8181 (note, we've set-up port 8181 for foreign node ui).  
 
From the terminal that ran the deployToken script, find the address the contract was deployed to under contractAddress, copy that value.  Back in the parity wallet ui, click the contracts tab, click + watch button, from there choose Custom Contract, next.  
 
On the Enter Contract Details from, paste tho contract address into the network address field.  Name the contract something like Test Token,  Then paste the abi for the Token contract into the contract abi field.  You'll find the contract abi in compiled_contracts/Token.abi.  Click add contract, and the watch will be added.
 
Now on the contracts tab you'll see the newly watched contract Test Token, click that to see details about this token.  Including the field isToken, which is true.  Our tokne has been deployed.
 
## Start Bridge

In an new terminal window, start the home node
`./parity_start_home_unlocked.sh`
 
 In another new terminal, start the bridge:
 `./bridge_start.sh`
You now have three terminals running, one parity home node, parity foreign node, and the bridge node.  The scripts started all these with info logging, so you’ll see rpc calls to the nodes, and other information.  

When the bridge starts it deploys the home and foreign bridge contracts.  You will now have a file called tmp/bridge1_db.txt, open that file to review contract addersses.
 
Add a watch to the foreign bridge contract, in the parity ui for foreign (http://127.0.0.1:8181), similar to the way we added the token watch.  Use the abi from the compiled_contracts/ForeignBridge.abi file.

## Token Setup

Now we need to set the token to use on the foreign bridge contract to the test token we deployed on the forgein node.  Edit the setToken script to change the tokenAddress to the address that the token was deployed to (without the 0x prefix).  And set the foreignContractAddress to the address of the foreign contract address.  Run the script `node setToken.js`
 
Back in browser, confirm that foreign bridge contract now has erc20token field set to test token address.
 
 Set the mintAgent for the test token, edit the setMintAgent.js script to set the tokenAddress to the address of the test token.  Run the script.
 `node setMintAgent.js`

Confirm this was set, in parity UI, on the Test Token contract, enter the address of the authority account (the one we created in the beginning 0x00bd13…) into the mintAgents address field, and click query, you should get a result of true.
 
At this point the test tokens address is connected to the foreign bridge contract, but there are no test tokens, so fund test foreign contract. Edit script fundForeignContract.js and enter foreignContractAddress and tokenAddress.  Run the script. 
 `node fundForeignContract.js`

Check token supply, contracts tab, select test token and note value in totalSupply field (4,294,967,295 eth)

Check that the foreign bridge contract has been funded, contracts tab, select test token, enter foreign bridge address in balanceOf address field, and click query, you should see a balance of a lot of test ether.

At this point the bridge is setup, now to actually test it.

## Testing
First confirm that the test development account has no test token in the foreign bridge contract.  On the contracts tab, select test token, in the balanceOf query, enter the address of the development account (0x00a329c0…), and click query. There should be 0 ether.

Send some ether te the home bridge
`./curl_deposit_to_home.sh`
This script sends as small amount of eth (0.0000000000001 eth) to the home bridge, now we should see that show up in balance of for test token on foreign chain.  In foreign parity ui, contracts, select test token then in the balanceOf enter the address of the development account, do a query and you'll see the balance for that address has increased.  You’ll also see activity on all the terminal windows indicating what has happened.

It worked!! We deposited eth to the home bridge contract on the home chain, the bridge detected this event and made the deposit of test tokens of that account address on the foreign chain.  

You can also use the script `node checkHomeBalance.js` to check current balance of ether for the user account on the home chain.  This script is handy for confirming when finds have entered or left the home user account.

Now, we need to withdraw funds from the foreign chain back onto the home chain, unfortantually I haven’t gotten that working yet.  

I have a script `node foreignWithdrawl.js` for testing this, and it looks like the transactions are working, but the funds aren’t being released.  Help!  I’ll work in getting that working, but let me know if you find a solution.
  
