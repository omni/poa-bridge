let utils = require("./utils");
let fs = require("fs")
let toml = require("toml")

let investorAddr = "0xEaeBA7869E23A328a0A92620BbA1A7a6aaED26cB"

//check that contract is created correctly: should return values
utils.getAuthorities("right");
utils.getRequiredSignatures("right");

//getTokenBalanceOf(investorAddr);

var dbtoml = fs.readFileSync('../examples/db.toml').toString('utf8');
var db = toml.parse(dbtoml);

//utils.getBalance("left", db.home_contract_address);
