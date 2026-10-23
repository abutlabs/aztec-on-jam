// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.27;

import {Test, console} from "forge-std/Test.sol";
import {HonkVerifier24_opt} from "../src/V24_opt.sol";

contract V24_optTest is Test {
    function test_verify() public {
        HonkVerifier24_opt verifier = new HonkVerifier24_opt();
        bytes memory proof = vm.readFileBinary("data/proof24");
        bytes memory raw = vm.readFileBinary("data/pubs24");
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
        console.log("2^24 opt verify gas:", used);
        pubs[0] = bytes32(uint256(pubs[0]) ^ 1);
        vm.expectRevert();
        verifier.verify(proof, pubs);
    }
}
