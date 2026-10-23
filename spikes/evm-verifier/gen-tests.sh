#!/usr/bin/env bash
# one test per verifier: each file declares its own IVerifier and libraries
cd "$(dirname "$0")"
for k in 16 20 24; do for v in opt plain; do cat > test/V${k}_$v.t.sol <<T
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.27;

import {Test, console} from "forge-std/Test.sol";
import {HonkVerifier${k}_$v} from "../src/V${k}_$v.sol";

contract V${k}_${v}Test is Test {
    function test_verify() public {
        HonkVerifier${k}_$v verifier = new HonkVerifier${k}_$v();
        bytes memory proof = vm.readFileBinary("data/proof$k");
        bytes memory raw = vm.readFileBinary("data/pubs$k");
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
        console.log("2^$k $v verify gas:", used);
        pubs[0] = bytes32(uint256(pubs[0]) ^ 1);
        vm.expectRevert();
        verifier.verify(proof, pubs);
    }
}
T
done; done
