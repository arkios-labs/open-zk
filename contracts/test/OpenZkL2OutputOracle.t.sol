// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {OpenZkL2OutputOracle} from "../src/OpenZkL2OutputOracle.sol";
import {MockSP1Verifier} from "../src/mock/MockSP1Verifier.sol";
import {MockRiscZeroVerifier} from "../src/mock/MockRiscZeroVerifier.sol";

contract OpenZkL2OutputOracleTest is Test {
    OpenZkL2OutputOracle oracle;
    MockSP1Verifier mockSp1;
    MockRiscZeroVerifier mockRisc0;

    bytes32 constant SP1_VKEY = bytes32(uint256(0x1234));
    bytes32 constant RISC0_IMAGE_ID = bytes32(uint256(0x5678));

    // Sample journal: 192 bytes (6 x 32-byte fields)
    bytes32 constant L1_HEAD = bytes32(uint256(0xaa));
    bytes32 constant L2_PRE_ROOT = bytes32(uint256(0xbb));
    bytes32 constant L2_POST_ROOT = bytes32(uint256(0xcc));
    uint256 constant L2_BLOCK_NUMBER = 42;
    bytes32 constant ROLLUP_CONFIG_HASH = bytes32(uint256(0xdd));
    bytes32 constant PROGRAM_ID = bytes32(uint256(0xee));

    function _sampleJournal() internal pure returns (bytes memory) {
        return abi.encodePacked(
            L1_HEAD,
            L2_PRE_ROOT,
            L2_POST_ROOT,
            bytes32(L2_BLOCK_NUMBER),
            ROLLUP_CONFIG_HASH,
            PROGRAM_ID
        );
    }

    function setUp() public {
        mockSp1 = new MockSP1Verifier();
        mockRisc0 = new MockRiscZeroVerifier();
        oracle = new OpenZkL2OutputOracle(
            address(mockSp1),
            address(mockRisc0),
            SP1_VKEY,
            RISC0_IMAGE_ID
        );
    }

    function test_submitSp1Proof() public {
        bytes memory journal = _sampleJournal();
        bytes memory fakeProof = hex"deadbeef";

        assertEq(oracle.latestBlockNumber(), 0);
        assertFalse(oracle.isBlockProven(42));

        oracle.submitSp1Proof(journal, fakeProof);

        assertTrue(oracle.isBlockProven(42));
        assertEq(oracle.latestBlockNumber(), 42);
        assertEq(oracle.latestOutputRoot(), L2_POST_ROOT);
    }

    function test_submitRisc0Proof() public {
        bytes memory journal = _sampleJournal();
        bytes memory fakeSeal = hex"cafebabe";

        oracle.submitRisc0Proof(journal, fakeSeal);

        assertTrue(oracle.isBlockProven(42));
        assertEq(oracle.latestBlockNumber(), 42);
        assertEq(oracle.latestOutputRoot(), L2_POST_ROOT);
    }

    function test_revert_blockAlreadyProven() public {
        bytes memory journal = _sampleJournal();
        oracle.submitSp1Proof(journal, hex"");

        vm.expectRevert(OpenZkL2OutputOracle.BlockAlreadyProven.selector);
        oracle.submitSp1Proof(journal, hex"");
    }

    function test_revert_invalidJournalLength() public {
        vm.expectRevert(OpenZkL2OutputOracle.InvalidJournalLength.selector);
        oracle.submitSp1Proof(hex"0000", hex"");
    }

    function test_latestBlockNumber_updates_only_forward() public {
        // Submit block 100 first
        bytes memory journal100 = abi.encodePacked(
            L1_HEAD, L2_PRE_ROOT, L2_POST_ROOT,
            bytes32(uint256(100)),
            ROLLUP_CONFIG_HASH, PROGRAM_ID
        );
        oracle.submitSp1Proof(journal100, hex"");
        assertEq(oracle.latestBlockNumber(), 100);

        // Submit block 50 — latestBlockNumber should stay at 100
        bytes memory journal50 = abi.encodePacked(
            L1_HEAD, L2_PRE_ROOT, bytes32(uint256(0xff)),
            bytes32(uint256(50)),
            ROLLUP_CONFIG_HASH, PROGRAM_ID
        );
        oracle.submitSp1Proof(journal50, hex"");
        assertEq(oracle.latestBlockNumber(), 100);
    }

    function test_setProgramIds_onlyOwner() public {
        oracle.setSp1ProgramVKey(bytes32(uint256(0x9999)));
        assertEq(oracle.sp1ProgramVKey(), bytes32(uint256(0x9999)));

        vm.prank(address(0xdead));
        vm.expectRevert(OpenZkL2OutputOracle.Unauthorized.selector);
        oracle.setSp1ProgramVKey(bytes32(uint256(0xaaaa)));
    }

    event OutputRootSubmitted(
        bytes32 indexed outputRoot,
        uint64 indexed l2BlockNumber,
        bytes32 l1Head,
        OpenZkL2OutputOracle.Backend backend
    );

    function test_emits_OutputRootSubmitted() public {
        bytes memory journal = _sampleJournal();

        vm.expectEmit(true, true, false, true);
        emit OutputRootSubmitted(
            L2_POST_ROOT,
            42,
            L1_HEAD,
            OpenZkL2OutputOracle.Backend.SP1
        );

        oracle.submitSp1Proof(journal, hex"");
    }
}
