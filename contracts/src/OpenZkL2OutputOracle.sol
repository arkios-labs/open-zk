// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {IRiscZeroVerifier} from "@risc0-ethereum/IRiscZeroVerifier.sol";

/// @title OpenZkL2OutputOracle
/// @notice Accepts ZK-proven L2 state transitions from SP1 or RISC Zero backends.
contract OpenZkL2OutputOracle {
    // -- Types ----------------------------------------------------------------

    enum Backend {
        SP1,
        RiscZero
    }

    struct OutputProposal {
        bytes32 outputRoot;
        uint64 l2BlockNumber;
        bytes32 l1Head;
        uint128 timestamp;
    }

    // -- State ----------------------------------------------------------------

    /// @notice SP1 verifier gateway (ISP1Verifier).
    address public immutable sp1Verifier;

    /// @notice RISC Zero verifier router (IRiscZeroVerifier).
    address public immutable risc0Verifier;

    /// @notice Allowed SP1 program verification key.
    bytes32 public sp1ProgramVKey;

    /// @notice Allowed RISC Zero image ID.
    bytes32 public risc0ImageId;

    /// @notice Contract owner (can update program IDs).
    address public owner;

    /// @notice Latest proven L2 block number.
    uint64 public latestBlockNumber;

    /// @notice Latest proven L2 output root.
    bytes32 public latestOutputRoot;

    /// @notice All output proposals by block number.
    mapping(uint64 => OutputProposal) public proposals;

    // -- Events ---------------------------------------------------------------

    event OutputRootSubmitted(
        bytes32 indexed outputRoot,
        uint64 indexed l2BlockNumber,
        bytes32 l1Head,
        Backend backend
    );

    event ProgramIdUpdated(Backend backend, bytes32 programId);

    // -- Errors ---------------------------------------------------------------

    error Unauthorized();
    error BlockAlreadyProven();
    error InvalidJournalLength();
    error VerificationFailed();

    // -- Constructor ----------------------------------------------------------

    constructor(
        address _sp1Verifier,
        address _risc0Verifier,
        bytes32 _sp1ProgramVKey,
        bytes32 _risc0ImageId
    ) {
        sp1Verifier = _sp1Verifier;
        risc0Verifier = _risc0Verifier;
        sp1ProgramVKey = _sp1ProgramVKey;
        risc0ImageId = _risc0ImageId;
        owner = msg.sender;
    }

    // -- External functions ---------------------------------------------------

    /// @notice Submit an SP1-proven state transition.
    /// @param publicValues ABI-encoded StateTransitionJournal (192 bytes).
    /// @param proofBytes   SP1 Groth16 proof bytes.
    function submitSp1Proof(
        bytes calldata publicValues,
        bytes calldata proofBytes
    ) external {
        if (publicValues.length != 192) revert InvalidJournalLength();

        // Verify via SP1 verifier gateway
        ISP1Verifier(sp1Verifier).verifyProof(
            sp1ProgramVKey,
            publicValues,
            proofBytes
        );

        _processJournal(publicValues, Backend.SP1);
    }

    /// @notice Submit a RISC Zero-proven state transition.
    /// @param journalBytes ABI-encoded StateTransitionJournal (192 bytes).
    /// @param seal          RISC Zero Groth16 seal.
    function submitRisc0Proof(
        bytes calldata journalBytes,
        bytes calldata seal
    ) external {
        if (journalBytes.length != 192) revert InvalidJournalLength();

        // Verify via RISC Zero verifier router
        IRiscZeroVerifier(risc0Verifier).verify(
            seal,
            risc0ImageId,
            sha256(journalBytes)
        );

        _processJournal(journalBytes, Backend.RiscZero);
    }

    /// @notice Check if a block has been proven.
    function isBlockProven(uint64 blockNumber) external view returns (bool) {
        return proposals[blockNumber].outputRoot != bytes32(0);
    }

    /// @notice Update the allowed SP1 program verification key.
    function setSp1ProgramVKey(bytes32 _vkey) external {
        if (msg.sender != owner) revert Unauthorized();
        sp1ProgramVKey = _vkey;
        emit ProgramIdUpdated(Backend.SP1, _vkey);
    }

    /// @notice Update the allowed RISC Zero image ID.
    function setRisc0ImageId(bytes32 _imageId) external {
        if (msg.sender != owner) revert Unauthorized();
        risc0ImageId = _imageId;
        emit ProgramIdUpdated(Backend.RiscZero, _imageId);
    }

    // -- Internal -------------------------------------------------------------

    /// @dev Decode journal, store the output proposal, update latest state.
    function _processJournal(
        bytes calldata journal,
        Backend backend
    ) internal {
        // Journal layout (192 bytes, ABI-encoded as 6 x bytes32):
        //   [  0: 32] l1_head
        //   [ 32: 64] l2_pre_root
        //   [ 64: 96] l2_post_root
        //   [ 96:128] l2_block_number (uint256, right-aligned)
        //   [128:160] rollup_config_hash
        //   [160:192] program_id
        bytes32 l1Head = bytes32(journal[0:32]);
        bytes32 l2PostRoot = bytes32(journal[64:96]);
        uint64 l2BlockNumber = uint64(uint256(bytes32(journal[96:128])));

        if (proposals[l2BlockNumber].outputRoot != bytes32(0))
            revert BlockAlreadyProven();

        proposals[l2BlockNumber] = OutputProposal({
            outputRoot: l2PostRoot,
            l2BlockNumber: l2BlockNumber,
            l1Head: l1Head,
            timestamp: uint128(block.timestamp)
        });

        if (l2BlockNumber > latestBlockNumber) {
            latestBlockNumber = l2BlockNumber;
            latestOutputRoot = l2PostRoot;
        }

        emit OutputRootSubmitted(l2PostRoot, l2BlockNumber, l1Head, backend);
    }
}
