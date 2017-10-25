let utils = require("./utils");
let fs = require("fs")
let toml = require("toml")

let investorAddr = "0x00dB9af45C6f241432F2cBE412c6969cB7778d98"

//check that contract is created correctly: should return values
utils.getAuthorities("right");
utils.getRequiredSignatures("right");

var config = utils.getConfig();
var dbtoml = fs.readFileSync('../examples/db.toml').toString('utf8');
var db = toml.parse(dbtoml);
//utils.getTokenBalanceOf(investorAddr);
utils.getERC20TokenBalanceOf(investorAddr, config.token.ABI, config.token.addr);
utils.getERC20TokenBalanceOf(db.foreign_contract_address, config.token.ABI, config.token.addr);


//utils.getBalance("left", db.home_contract_address);