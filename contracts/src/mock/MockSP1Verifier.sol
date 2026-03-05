// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";

/// @title MockSP1Verifier
/// @notice Always-passing SP1 verifier for devnet/testing.
contract MockSP1Verifier is ISP1Verifier {
    function verifyProof(
        bytes32,
        bytes calldata,
        bytes calldata
    ) external pure override {
        // Always passes — no real verification.
    }
}
