// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {ISP1Verifier} from "@sp1-contracts/ISP1Verifier.sol";
import {IRiscZeroVerifier} from "@risc0-ethereum/IRiscZeroVerifier.sol";
import {OpenZkL2OutputOracle} from "./OpenZkL2OutputOracle.sol";

/// @title OpenZkDisputeGame
/// @notice Manages disputes on L2 output roots. Anyone can challenge a proven
///         output root; a ZK proof resolves the dispute.
contract OpenZkDisputeGame {
    // -- Types ----------------------------------------------------------------

    enum Backend {
        SP1,
        RiscZero
    }

    struct Dispute {
        address challenger;
        uint64 blockNumber;
        bytes32 disputedOutputRoot;
        uint128 createdAt;
        bool resolved;
        bool valid; // true = original output was correct
    }

    // -- State ----------------------------------------------------------------

    /// @notice The L2OutputOracle whose output roots can be disputed.
    OpenZkL2OutputOracle public immutable oracle;

    /// @notice SP1 verifier gateway.
    address public immutable sp1Verifier;

    /// @notice RISC Zero verifier router.
    address public immutable risc0Verifier;

    /// @notice Allowed SP1 program verification key.
    bytes32 public sp1ProgramVKey;

    /// @notice Allowed RISC Zero image ID.
    bytes32 public risc0ImageId;

    /// @notice Contract owner (can update program IDs).
    address public owner;

    /// @notice Duration (in seconds) within which a dispute must be resolved.
    uint256 public immutable challengeTimeout;

    /// @notice All disputes by block number.
    mapping(uint64 => Dispute) public disputes;

    // -- Events ---------------------------------------------------------------

    event DisputeCreated(
        uint64 indexed blockNumber,
        address indexed challenger,
        bytes32 disputedOutputRoot
    );

    event DisputeResolved(
        uint64 indexed blockNumber,
        bool valid,
        Backend backend
    );

    event ProgramIdUpdated(Backend backend, bytes32 programId);

    // -- Errors ---------------------------------------------------------------

    error Unauthorized();
    error BlockNotProven();
    error AlreadyDisputed();
    error DisputeNotFound();
    error AlreadyResolved();
    error InvalidJournalLength();
    error InvalidBackend();

    // -- Constructor ----------------------------------------------------------

    constructor(
        address _oracle,
        address _sp1Verifier,
        address _risc0Verifier,
        bytes32 _sp1ProgramVKey,
        bytes32 _risc0ImageId,
        uint256 _challengeTimeout
    ) {
        oracle = OpenZkL2OutputOracle(_oracle);
        sp1Verifier = _sp1Verifier;
        risc0Verifier = _risc0Verifier;
        sp1ProgramVKey = _sp1ProgramVKey;
        risc0ImageId = _risc0ImageId;
        challengeTimeout = _challengeTimeout;
        owner = msg.sender;
    }

    // -- External functions ---------------------------------------------------

    /// @notice Challenge a proven output root.
    /// @param blockNumber The L2 block number whose output root is being disputed.
    function challenge(uint64 blockNumber) external {
        // Verify the block was actually proven in the oracle
        if (!oracle.isBlockProven(blockNumber)) revert BlockNotProven();

        // Prevent double-dispute
        if (disputes[blockNumber].createdAt != 0) revert AlreadyDisputed();

        (bytes32 outputRoot, , , ) = oracle.proposals(blockNumber);

        disputes[blockNumber] = Dispute({
            challenger: msg.sender,
            blockNumber: blockNumber,
            disputedOutputRoot: outputRoot,
            createdAt: uint128(block.timestamp),
            resolved: false,
            valid: false
        });

        emit DisputeCreated(blockNumber, msg.sender, outputRoot);
    }

    /// @notice Resolve a dispute by submitting a ZK proof of the correct state transition.
    /// @param blockNumber The disputed L2 block number.
    /// @param publicValues ABI-encoded StateTransitionJournal (192 bytes).
    /// @param proofBytes Proof bytes (SP1 Groth16 or RISC Zero seal).
    /// @param backend 0 = SP1, 1 = RiscZero.
    function resolve(
        uint64 blockNumber,
        bytes calldata publicValues,
        bytes calldata proofBytes,
        uint8 backend
    ) external {
        Dispute storage dispute = disputes[blockNumber];
        if (dispute.createdAt == 0) revert DisputeNotFound();
        if (dispute.resolved) revert AlreadyResolved();
        if (publicValues.length != 192) revert InvalidJournalLength();

        // Verify the proof
        if (backend == uint8(Backend.SP1)) {
            ISP1Verifier(sp1Verifier).verifyProof(
                sp1ProgramVKey,
                publicValues,
                proofBytes
            );
        } else if (backend == uint8(Backend.RiscZero)) {
            IRiscZeroVerifier(risc0Verifier).verify(
                proofBytes,
                risc0ImageId,
                sha256(publicValues)
            );
        } else {
            revert InvalidBackend();
        }

        // Decode l2_post_root from journal (bytes 64..96)
        bytes32 provenOutputRoot = bytes32(publicValues[64:96]);

        // Mark resolved before any further state changes
        dispute.resolved = true;
        dispute.valid = (provenOutputRoot == dispute.disputedOutputRoot);

        emit DisputeResolved(
            blockNumber,
            dispute.valid,
            Backend(backend)
        );
    }

    /// @notice Check if a block is currently under dispute.
    function isDisputed(uint64 blockNumber) external view returns (bool) {
        return disputes[blockNumber].createdAt != 0
            && !disputes[blockNumber].resolved;
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
}
