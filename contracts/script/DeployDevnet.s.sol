// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import {OpenZkL2OutputOracle} from "../src/OpenZkL2OutputOracle.sol";
import {MockSP1Verifier} from "../src/mock/MockSP1Verifier.sol";
import {MockRiscZeroVerifier} from "../src/mock/MockRiscZeroVerifier.sol";

/// @notice Deploy mock verifiers + OpenZkL2OutputOracle to devnet.
contract DeployDevnet is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");

        // Program IDs can be overridden via env; defaults to zero for devnet.
        bytes32 sp1VKey = vm.envOr("SP1_PROGRAM_VKEY", bytes32(0));
        bytes32 risc0ImageId = vm.envOr("RISC0_IMAGE_ID", bytes32(0));

        vm.startBroadcast(deployerKey);

        MockSP1Verifier mockSp1 = new MockSP1Verifier();
        MockRiscZeroVerifier mockRisc0 = new MockRiscZeroVerifier();

        OpenZkL2OutputOracle oracle = new OpenZkL2OutputOracle(
            address(mockSp1),
            address(mockRisc0),
            sp1VKey,
            risc0ImageId
        );

        vm.stopBroadcast();

        console.log("MockSP1Verifier:        ", address(mockSp1));
        console.log("MockRiscZeroVerifier:    ", address(mockRisc0));
        console.log("OpenZkL2OutputOracle:    ", address(oracle));
    }
}
