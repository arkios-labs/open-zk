// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {IRiscZeroVerifier, Receipt} from "@risc0-ethereum/IRiscZeroVerifier.sol";

/// @title MockRiscZeroVerifier
/// @notice Always-passing RISC Zero verifier for devnet/testing.
contract MockRiscZeroVerifier is IRiscZeroVerifier {
    function verify(
        bytes calldata,
        bytes32,
        bytes32
    ) external pure override {
        // Always passes — no real verification.
    }

    function verifyIntegrity(
        Receipt calldata
    ) external pure override {
        // Always passes.
    }
}
