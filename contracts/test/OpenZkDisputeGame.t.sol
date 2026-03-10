// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import {OpenZkL2OutputOracle} from "../src/OpenZkL2OutputOracle.sol";
import {OpenZkDisputeGame} from "../src/OpenZkDisputeGame.sol";
import {MockSP1Verifier} from "../src/mock/MockSP1Verifier.sol";
import {MockRiscZeroVerifier} from "../src/mock/MockRiscZeroVerifier.sol";

contract OpenZkDisputeGameTest is Test {
    OpenZkL2OutputOracle oracle;
    OpenZkDisputeGame disputeGame;
    MockSP1Verifier mockSp1;
    MockRiscZeroVerifier mockRisc0;

    bytes32 constant SP1_VKEY = bytes32(uint256(0x1234));
    bytes32 constant RISC0_IMAGE_ID = bytes32(uint256(0x5678));
    uint256 constant CHALLENGE_TIMEOUT = 3600;

    // Sample journal fields
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

        disputeGame = new OpenZkDisputeGame(
            address(oracle),
            address(mockSp1),
            address(mockRisc0),
            SP1_VKEY,
            RISC0_IMAGE_ID,
            CHALLENGE_TIMEOUT
        );

        // Submit a proof to oracle so there's something to dispute
        oracle.submitSp1Proof(_sampleJournal(), hex"deadbeef");
    }

    function test_challenge() public {
        assertFalse(disputeGame.isDisputed(42));

        disputeGame.challenge(42);

        assertTrue(disputeGame.isDisputed(42));
    }

    event DisputeCreated(
        uint64 indexed blockNumber,
        address indexed challenger,
        bytes32 disputedOutputRoot
    );

    event DisputeResolved(
        uint64 indexed blockNumber,
        bool valid,
        OpenZkDisputeGame.Backend backend
    );

    function test_challenge_emits_event() public {
        vm.expectEmit(true, true, false, true);
        emit DisputeCreated(42, address(this), L2_POST_ROOT);

        disputeGame.challenge(42);
    }

    function test_challenge_revert_blockNotProven() public {
        vm.expectRevert(OpenZkDisputeGame.BlockNotProven.selector);
        disputeGame.challenge(99); // block 99 was never proven
    }

    function test_challenge_revert_alreadyDisputed() public {
        disputeGame.challenge(42);

        vm.expectRevert(OpenZkDisputeGame.AlreadyDisputed.selector);
        disputeGame.challenge(42);
    }

    function test_resolve_sp1_valid() public {
        disputeGame.challenge(42);

        // Resolve with a proof whose l2_post_root matches the oracle's output root
        bytes memory journal = _sampleJournal();
        disputeGame.resolve(42, journal, hex"deadbeef", 0); // 0 = SP1

        assertFalse(disputeGame.isDisputed(42)); // resolved
        (, , , , bool resolved, bool valid) = disputeGame.disputes(42);
        assertTrue(resolved);
        assertTrue(valid);
    }

    function test_resolve_risc0_valid() public {
        disputeGame.challenge(42);

        bytes memory journal = _sampleJournal();
        disputeGame.resolve(42, journal, hex"cafebabe", 1); // 1 = RiscZero

        (, , , , bool resolved, bool valid) = disputeGame.disputes(42);
        assertTrue(resolved);
        assertTrue(valid);
    }

    function test_resolve_invalid_output() public {
        disputeGame.challenge(42);

        // Journal with a different l2_post_root
        bytes memory journal = abi.encodePacked(
            L1_HEAD,
            L2_PRE_ROOT,
            bytes32(uint256(0xff)), // different post root
            bytes32(L2_BLOCK_NUMBER),
            ROLLUP_CONFIG_HASH,
            PROGRAM_ID
        );

        disputeGame.resolve(42, journal, hex"deadbeef", 0);

        (, , , , bool resolved, bool valid) = disputeGame.disputes(42);
        assertTrue(resolved);
        assertFalse(valid); // challenger was right
    }

    function test_resolve_emits_event() public {
        disputeGame.challenge(42);

        vm.expectEmit(true, false, false, true);
        emit DisputeResolved(
            42,
            true,
            OpenZkDisputeGame.Backend.SP1
        );

        disputeGame.resolve(42, _sampleJournal(), hex"deadbeef", 0);
    }

    function test_resolve_revert_disputeNotFound() public {
        vm.expectRevert(OpenZkDisputeGame.DisputeNotFound.selector);
        disputeGame.resolve(42, _sampleJournal(), hex"", 0);
    }

    function test_resolve_revert_alreadyResolved() public {
        disputeGame.challenge(42);
        disputeGame.resolve(42, _sampleJournal(), hex"", 0);

        vm.expectRevert(OpenZkDisputeGame.AlreadyResolved.selector);
        disputeGame.resolve(42, _sampleJournal(), hex"", 0);
    }

    function test_resolve_revert_invalidJournalLength() public {
        disputeGame.challenge(42);

        vm.expectRevert(OpenZkDisputeGame.InvalidJournalLength.selector);
        disputeGame.resolve(42, hex"0000", hex"", 0);
    }

    function test_resolve_revert_invalidBackend() public {
        disputeGame.challenge(42);

        vm.expectRevert(OpenZkDisputeGame.InvalidBackend.selector);
        disputeGame.resolve(42, _sampleJournal(), hex"", 99);
    }

    function test_setProgramIds_onlyOwner() public {
        disputeGame.setSp1ProgramVKey(bytes32(uint256(0x9999)));
        assertEq(disputeGame.sp1ProgramVKey(), bytes32(uint256(0x9999)));

        vm.prank(address(0xdead));
        vm.expectRevert(OpenZkDisputeGame.Unauthorized.selector);
        disputeGame.setSp1ProgramVKey(bytes32(uint256(0xaaaa)));
    }
}
