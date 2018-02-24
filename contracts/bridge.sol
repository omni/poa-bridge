pragma solidity ^0.4.19;

/// general helpers.
/// `internal` so they get compiled into contracts using them.
library Helpers {
    /// returns whether `array` contains `value`.
    function addressArrayContains(address[] array, address value) internal pure returns (bool) {
        for (uint256 i = 0; i < array.length; i++) {
            if (array[i] == value) {
                return true;
            }
        }
        return false;
    }

    // returns the digits of `inputValue` as a string.
    // example: `uintToString(12345678)` returns `"12345678"`
    function uintToString(uint256 inputValue) internal pure returns (string) {
        // figure out the length of the resulting string
        uint256 length = 0;
        uint256 currentValue = inputValue;
        do {
            length++;
            currentValue /= 10;
        } while (currentValue != 0);
        // allocate enough memory
        bytes memory result = new bytes(length);
        // construct the string backwards
        uint256 i = length - 1;
        currentValue = inputValue;
        do {
            result[i--] = byte(48 + currentValue % 10);
            currentValue /= 10;
        } while (currentValue != 0);
        return string(result);
    }

    /// returns whether signatures (whose components are in `vs`, `rs`, `ss`)
    /// contain `requiredSignatures` distinct correct signatures
    /// where signer is in `allowed_signers`
    /// that signed `message`
    function hasEnoughValidSignatures(bytes message, uint8[] vs, bytes32[] rs, bytes32[] ss, address[] allowed_signers, uint256 requiredSignatures) internal pure returns (bool) {
        // not enough signatures
        if (vs.length < requiredSignatures) {
            return false;
        }

        var hash = MessageSigning.hashMessage(message);
        var encountered_addresses = new address[](allowed_signers.length);

        for (uint256 i = 0; i < requiredSignatures; i++) {
            var recovered_address = ecrecover(hash, vs[i], rs[i], ss[i]);
            // only signatures by addresses in `addresses` are allowed
            if (!addressArrayContains(allowed_signers, recovered_address)) {
                return false;
            }
            // duplicate signatures are not allowed
            if (addressArrayContains(encountered_addresses, recovered_address)) {
                return false;
            }
            encountered_addresses[i] = recovered_address;
        }
        return true;
    }

}


/// Library used only to test Helpers library via rpc calls
library HelpersTest {
    function addressArrayContains(address[] array, address value) public pure returns (bool) {
        return Helpers.addressArrayContains(array, value);
    }

    function uintToString(uint256 inputValue) public pure returns (string str) {
        return Helpers.uintToString(inputValue);
    }

    function hasEnoughValidSignatures(bytes message, uint8[] vs, bytes32[] rs, bytes32[] ss, address[] addresses, uint256 requiredSignatures) public pure returns (bool) {
        return Helpers.hasEnoughValidSignatures(message, vs, rs, ss, addresses, requiredSignatures);
    }
}


// helpers for message signing.
// `internal` so they get compiled into contracts using them.
library MessageSigning {
    function recoverAddressFromSignedMessage(bytes signature, bytes message) internal pure returns (address) {
        require(signature.length == 65);
        bytes32 r;
        bytes32 s;
        bytes1 v;
        // solium-disable-next-line security/no-inline-assembly
        assembly {
            r := mload(add(signature, 0x20))
            s := mload(add(signature, 0x40))
            v := mload(add(signature, 0x60))
        }
        return ecrecover(hashMessage(message), uint8(v), r, s);
    }

    function hashMessage(bytes message) internal pure returns (bytes32) {
        bytes memory prefix = "\x19Ethereum Signed Message:\n";
        return keccak256(prefix, Helpers.uintToString(message.length), message);
    }
}


/// Library used only to test MessageSigning library via rpc calls
library MessageSigningTest {
    function recoverAddressFromSignedMessage(bytes signature, bytes message) public pure returns (address) {
        return MessageSigning.recoverAddressFromSignedMessage(signature, message);
    }
}


library Message {
    // layout of message :: bytes:
    // offset  0: 32 bytes :: uint256 - message length
    // offset 32: 20 bytes :: address - recipient address
    // offset 52: 32 bytes :: uint256 - value
    // offset 84: 32 bytes :: bytes32 - transaction hash
    // offset 116: 32 bytes :: uint256 - home gas price

    // bytes 1 to 32 are 0 because message length is stored as little endian.
    // mload always reads 32 bytes.
    // so we can and have to start reading recipient at offset 20 instead of 32.
    // if we were to read at 32 the address would contain part of value and be corrupted.
    // when reading from offset 20 mload will read 12 zero bytes followed
    // by the 20 recipient address bytes and correctly convert it into an address.
    // this saves some storage/gas over the alternative solution
    // which is padding address to 32 bytes and reading recipient at offset 32.
    // for more details see discussion in:
    // https://github.com/paritytech/parity-bridge/issues/61

    function getRecipient(bytes message) internal pure returns (address) {
        address recipient;
        // solium-disable-next-line security/no-inline-assembly
        assembly {
            recipient := mload(add(message, 20))
        }
        return recipient;
    }

    function getValue(bytes message) internal pure returns (uint256) {
        uint256 value;
        // solium-disable-next-line security/no-inline-assembly
        assembly {
            value := mload(add(message, 52))
        }
        return value;
    }

    function getTransactionHash(bytes message) internal pure returns (bytes32) {
        bytes32 hash;
        // solium-disable-next-line security/no-inline-assembly
        assembly {
            hash := mload(add(message, 84))
        }
        return hash;
    }

    function getHomeGasPrice(bytes message) internal pure returns (uint256) {
        uint256 gasPrice;
        // solium-disable-next-line security/no-inline-assembly
        assembly {
            gasPrice := mload(add(message, 116))
        }
        return gasPrice;
    }
}


/// Library used only to test Message library via rpc calls
library MessageTest {
    function getRecipient(bytes message) public pure returns (address) {
        return Message.getRecipient(message);
    }

    function getValue(bytes message) public pure returns (uint256) {
        return Message.getValue(message);
    }

    function getTransactionHash(bytes message) public pure returns (bytes32) {
        return Message.getTransactionHash(message);
    }

    function getHomeGasPrice(bytes message) public pure returns (uint256) {
        return Message.getHomeGasPrice(message);
    }
}


contract HomeBridge {
    /// Number of authorities signatures required to withdraw the money.
    ///
    /// Must be lesser than number of authorities.
    uint256 public requiredSignatures;

    /// The gas cost of calling `HomeBridge.withdraw`.
    ///
    /// Is subtracted from `value` on withdraw.
    /// recipient pays the relaying authority for withdraw.
    /// this shuts down attacks that exhaust authorities funds on home chain.
    uint256 public estimatedGasCostOfWithdraw;

    /// Contract authorities.
    address[] public authorities;

    /// Used foreign transaction hashes.
    mapping (bytes32 => bool) withdraws;

    /// Event created on money deposit.
    event Deposit (address recipient, uint256 value);

    /// Event created on money withdraw.
    event Withdraw (address recipient, uint256 value);

    /// Constructor.
    function HomeBridge(
        uint256 requiredSignaturesParam,
        address[] authoritiesParam,
        uint256 estimatedGasCostOfWithdrawParam
    ) public
    {
        require(requiredSignaturesParam != 0);
        require(requiredSignaturesParam <= authoritiesParam.length);
        requiredSignatures = requiredSignaturesParam;
        authorities = authoritiesParam;
        estimatedGasCostOfWithdraw = estimatedGasCostOfWithdrawParam;
    }

    /// Should be used to deposit money.
    function () public payable {
        Deposit(msg.sender, msg.value);
    }

    /// final step of a withdraw.
    /// checks that `requiredSignatures` `authorities` have signed of on the `message`.
    /// then transfers `value` to `recipient` (both extracted from `message`).
    /// see message library above for a breakdown of the `message` contents.
    /// `vs`, `rs`, `ss` are the components of the signatures.

    /// anyone can call this, provided they have the message and required signatures!
    /// only the `authorities` can create these signatures.
    /// `requiredSignatures` authorities can sign arbitrary `message`s
    /// transfering any ether `value` out of this contract to `recipient`.
    /// bridge users must trust a majority of `requiredSignatures` of the `authorities`.
    function withdraw(uint8[] vs, bytes32[] rs, bytes32[] ss, bytes message) public {
        require(message.length == 116);

        // check that at least `requiredSignatures` `authorities` have signed `message`
        require(Helpers.hasEnoughValidSignatures(message, vs, rs, ss, authorities, requiredSignatures));

        address recipient = Message.getRecipient(message);
        uint256 value = Message.getValue(message);
        bytes32 hash = Message.getTransactionHash(message);
        uint256 homeGasPrice = Message.getHomeGasPrice(message);

        // if the recipient calls `withdraw` they can choose the gas price freely.
        // if anyone else calls `withdraw` they have to use the gas price
        // `homeGasPrice` specified by the user initiating the withdraw.
        // this is a security mechanism designed to shut down
        // malicious senders setting extremely high gas prices
        // and effectively burning recipients withdrawn value.
        // see https://github.com/paritytech/parity-bridge/issues/112
        // for further explanation.
        require((recipient == msg.sender) || (tx.gasprice == homeGasPrice));

        // The following two statements guard against reentry into this function.
        // Duplicated withdraw or reentry.
        require(!withdraws[hash]);
        // Order of operations below is critical to avoid TheDAO-like re-entry bug
        withdraws[hash] = true;

        uint256 estimatedWeiCostOfWithdraw = estimatedGasCostOfWithdraw * homeGasPrice;

        // charge recipient for relay cost
        uint256 valueRemainingAfterSubtractingCost = value - estimatedWeiCostOfWithdraw;

        // pay out recipient
        recipient.transfer(valueRemainingAfterSubtractingCost);

        // refund relay cost to relaying authority
        msg.sender.transfer(estimatedWeiCostOfWithdraw);

        Withdraw(recipient, valueRemainingAfterSubtractingCost);
    }
}

contract ERC20 {
    function transfer(address to, uint256 value) public returns (bool);
    function transferFrom(address from, address to, uint256 value) public returns (bool);
    function allowance(address owner, address spender) public constant returns (uint256);
}

contract ForeignBridge {
    /// Number of authorities signatures required to withdraw the money.
    ///
    /// Must be less than number of authorities.
    uint256 public requiredSignatures;

    uint256 public estimatedGasCostOfWithdraw;

    // Original parity-bridge assumes that anyone could forward final
    // withdraw confirmation to the HomeBridge contract. That's why
    // they need to make sure that no one is trying to steal funds by
    // setting a big gas price of withdraw transaction. So,
    // funds sender is responsible to limit this by setting gasprice
    // as part of withdraw request.
    // Since it is not the case for POA CCT bridge, gasprice is set
    // to 1 Gwei which is minimal gasprice for POA network.
    uint256 homeGasPrice = 1000000000 wei;

    /// Contract authorities.
    mapping (address => bool) authorities;

    /// Pending mesages
    mapping (bytes32 => bytes) messages;
    /// ???
    mapping (bytes32 => bytes) signatures;
    
    /// Pending deposits and authorities who confirmed them
    mapping (bytes32 => bool) messages_signed;
    mapping (bytes32 => uint) num_messages_signed;

    /// Pending deposits and authorities who confirmed them
    mapping (bytes32 => bool) deposits_signed;
    mapping (bytes32 => uint) num_deposits_signed;

    /// Token to work with
    ERC20 public erc20token;

    /// List of authorities confirmed to set up ERC-20 token address
    mapping (bytes32 => bool) tokenAddressAprroval_signs;
    mapping (address => uint256) num_tokenAddressAprroval_signs;

    /// triggered when relay of deposit from HomeBridge is complete
    event Deposit(address recipient, uint256 value);

    /// Event created on money withdraw.
    event Withdraw(address recipient, uint256 value, uint256 homeGasPrice);

    /// Collected signatures which should be relayed to home chain.
    event CollectedSignatures(address authorityResponsibleForRelay, bytes32 messageHash);

    /// Event created when new token address is set up.
    event TokenAddress(address token);

    /// Constructor.
    function ForeignBridge(
        uint256 _requiredSignatures,
        address[] _authorities,
        uint256 _estimatedGasCostOfWithdraw
    ) public
    {
        require(_requiredSignatures != 0);
        require(_requiredSignatures <= _authorities.length);
        requiredSignatures = _requiredSignatures;
        
        for (uint i = 0; i < _authorities.length; i++) {
            authorities[_authorities[i]] = true;
        }
        
        estimatedGasCostOfWithdraw = _estimatedGasCostOfWithdraw;
    }

    /// require that sender is an authority
    modifier onlyAuthority() {
        require(authorities[msg.sender]);
        _;
    }

    /// Set up the token address. It allows to set up or change
    /// the ERC20 token address only if authorities confirmed this.
    ///
    /// Usage maps instead of arrey allows to reduce gas consumption
    ///
    /// token address (address)
    function setTokenAddress (ERC20 token) public onlyAuthority() {
        // Duplicated deposits
        bytes32 token_sender = keccak256(msg.sender, token);
        require(!tokenAddressAprroval_signs[token_sender]);
        tokenAddressAprroval_signs[token_sender]= true;

        uint signed = num_tokenAddressAprroval_signs[address(token)] + 1;
        num_tokenAddressAprroval_signs[address(token)] = signed;

        // TODO: this may cause troubles if requriedSignatures len is changed
        if (signed == requiredSignatures) {
            erc20token = ERC20(token);
            TokenAddress(token);
        }
    }

    /// Used to transfer tokens to the `recipient`.
    /// The bridge contract must own enough tokens to release them for 
    /// recipients. Tokens must be transfered to the bridge contract BEFORE
    /// the first deposit will be performed.
    ///
    /// Usage maps instead of array allows to reduce gas consumption
    /// from 91169 to 89348 (solc 0.4.19). 
    ///
    /// deposit recipient (bytes20)
    /// deposit value (uint256)
    /// mainnet transaction hash (bytes32) // to avoid transaction duplication
    function deposit(address recipient, uint value, bytes32 transactionHash) public onlyAuthority() {
        require(erc20token != address(0x0));

        // Protection from misbehaing authority
        bytes32 hash_msg = keccak256(recipient, value, transactionHash);
        bytes32 hash_sender = keccak256(msg.sender, hash_msg);

        // Duplicated deposits
        require(!deposits_signed[hash_sender]);
        deposits_signed[hash_sender]= true;

        uint signed = num_deposits_signed[hash_msg] + 1;
        num_deposits_signed[hash_msg] = signed;

        // TODO: this may cause troubles if requriedSignatures len is changed
        if (signed == requiredSignatures) {
            // If the bridge contract does not own enough tokens to transfer
            // it will couse funds lock on the home side of the bridge
            erc20token.transfer(recipient, value);
            Deposit(recipient, value);
        }
    }

    /// Used to transfer `value` of tokens from `_from`s balance on local 
    /// (`foreign`) chain to the same address (`_from`) on `home` chain.
    /// Transfer of tokens within local (`foreign`) chain performed by usual
    /// way through transfer method of the token contract.
    /// In order to swap tokens to coins the owner (`_from`) must allow this
    /// explicitly in the token contract by calling approveAndCall with address
    /// of the bridge account.
    /// The method locks tokens and emits a `Withdraw` event which will be
    /// picked up by the bridge authorities.
    /// Bridge authorities will then sign off (by calling `submitSignature`) on
    /// a message containing `value`, the recipient (`_from`) and the `hash` of
    /// the transaction on `foreign` containing the `Withdraw` event.
    /// Once `requiredSignatures` are collected a `CollectedSignatures` event
    /// will be emitted.
    /// An authority will pick up `CollectedSignatures` an call
    /// `HomeBridge.withdraw` which transfers `value - relayCost` to the
    /// recipient completing the transfer.
    function receiveApproval(address _from, uint256 _value, ERC20 _tokenContract, bytes _msg) external returns(bool) {
        require(erc20token != address(0x0));
        require(msg.sender == address(erc20token));
        require(erc20token.allowance(_from, this) >= _value);
        erc20token.transferFrom(_from, this, _value);
        Withdraw(_from, _value, homeGasPrice);

        return true;
    }

    /// Should be used as sync tool
    ///
    /// Message is a message that should be relayed to main chain once authorities sign it.
    ///
    /// Usage several maps instead of structure allows to reduce gas consumption
    /// from 265102 to 242334 (solc 0.4.19). 
    ///
    /// for withdraw message contains:
    /// withdrawal recipient (bytes20)
    /// withdrawal value (uint256)
    /// foreign transaction hash (bytes32) // to avoid transaction duplication
    function submitSignature(bytes signature, bytes message) public onlyAuthority() {
        // ensure that `signature` is really `message` signed by `msg.sender`
        require(msg.sender == MessageSigning.recoverAddressFromSignedMessage(signature, message));

        require(message.length == 116);
        bytes32 hash = keccak256(message);
        bytes32 hash_sender = keccak256(msg.sender, hash);

        uint signed = num_messages_signed[hash_sender] + 1;

        if (signed > 1) {
            // Duplicated signatures
            require(!messages_signed[hash_sender]);
        }
        else {
            // check if it will really reduce gas usage in case of the second transaction
            // with the same hash
            messages[hash] = message;
        }
        messages_signed[hash_sender] = true;

        bytes32 sign_idx = keccak256(hash, (signed-1));
        signatures[sign_idx]= signature;

        num_messages_signed[hash_sender] = signed;

        // TODO: this may cause troubles if requiredSignatures len is changed
        if (signed == requiredSignatures) {
            CollectedSignatures(msg.sender, hash);
        }
    }

    /// Get signature
    function signature(bytes32 hash, uint index) public view returns (bytes) {
        bytes32 sign_idx = keccak256(hash, index);
        return signatures[sign_idx];
    }

    /// Get message
    function message(bytes32 hash) public view returns (bytes) {
        return messages[hash];
    }
}
