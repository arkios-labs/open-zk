// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import {OpenZkL2OutputOracle} from "../src/OpenZkL2OutputOracle.sol";
import {OpenZkDisputeGame} from "../src/OpenZkDisputeGame.sol";
import {MockSP1Verifier} from "../src/mock/MockSP1Verifier.sol";
import {MockRiscZeroVerifier} from "../src/mock/MockRiscZeroVerifier.sol";

/// @notice Deploy mock verifiers + OpenZkL2OutputOracle (+ optional DisputeGame) to devnet.
contract DeployDevnet is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");

        // Program IDs can be overridden via env; defaults to zero for devnet.
        bytes32 sp1VKey = vm.envOr("SP1_PROGRAM_VKEY", bytes32(0));
        bytes32 risc0ImageId = vm.envOr("RISC0_IMAGE_ID", bytes32(0));

        // DisputeGame config
        bool deployDispute = vm.envOr("DEPLOY_DISPUTE_GAME", false);
        uint256 challengeTimeout = vm.envOr("CHALLENGE_TIMEOUT", uint256(3600));

        vm.startBroadcast(deployerKey);

        MockSP1Verifier mockSp1 = new MockSP1Verifier();
        MockRiscZeroVerifier mockRisc0 = new MockRiscZeroVerifier();

        OpenZkL2OutputOracle oracle = new OpenZkL2OutputOracle(
            address(mockSp1),
            address(mockRisc0),
            sp1VKey,
            risc0ImageId
        );

        if (deployDispute) {
            OpenZkDisputeGame disputeGame = new OpenZkDisputeGame(
                address(oracle),
                address(mockSp1),
                address(mockRisc0),
                sp1VKey,
                risc0ImageId,
                challengeTimeout
            );
            console.log("OpenZkDisputeGame:      ", address(disputeGame));
            console.log("  challengeTimeout:      ", challengeTimeout);
        }

        vm.stopBroadcast();

        console.log("MockSP1Verifier:        ", address(mockSp1));
        console.log("MockRiscZeroVerifier:    ", address(mockRisc0));
        console.log("OpenZkL2OutputOracle:    ", address(oracle));
    }
}
