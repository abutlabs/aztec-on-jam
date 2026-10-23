// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.27;

import {Test, console} from "forge-std/Test.sol";
import {HonkVerifier20_plain} from "../src/V20_plain.sol";

contract V20_plainTest is Test {
    function test_verify() public {
        HonkVerifier20_plain verifier = new HonkVerifier20_plain();
        bytes memory proof = vm.readFileBinary("data/proof20");
        bytes memory raw = vm.readFileBinary("data/pubs20");
        bytes32[] memory pubs = new bytes32[](raw.length / 32);
        for (uint256 i = 0; i < pubs.length; i++) {
            bytes32 w;
            assembly { w := mload(add(add(raw, 32), mul(i, 32))) }
            pubs[i] = w;
        }
        uint256 before = gasleft();
        bool ok = verifier.verify(proof, pubs);
        uint256 used = before - gasleft();
        assertTrue(ok);
        console.log("2^20 plain verify gas:", used);
        pubs[0] = bytes32(uint256(pubs[0]) ^ 1);
        vm.expectRevert();
        verifier.verify(proof, pubs);
    }
}
