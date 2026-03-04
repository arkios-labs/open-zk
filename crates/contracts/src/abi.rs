//! Solidity ABI definitions for on-chain verifier and oracle contracts.

use alloy_sol_types::sol;

sol! {
    /// Verifier contract interface for validating zkVM proofs.
    interface IProofVerifier {
        /// Verify a proof against a program verification key and public values.
        function verifyProof(
            bytes32 programVKey,
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external view;
    }

    /// L2 Output Oracle that accepts proven state transitions.
    interface IOpenZkL2OutputOracle {
        /// Submit a verified proof of an L2 state transition.
        function submitProof(
            bytes32 l1Head,
            bytes32 l2PreRoot,
            bytes32 l2PostRoot,
            uint64 l2BlockNumber,
            bytes32 rollupConfigHash,
            bytes32 programId,
            bytes calldata proof
        ) external;

        /// Get the latest proven L2 output root.
        function latestOutputRoot() external view returns (bytes32);

        /// Get the latest proven L2 block number.
        function latestBlockNumber() external view returns (uint64);

        /// Check if a specific block has been proven.
        function isBlockProven(uint64 blockNumber) external view returns (bool);

        /// Emitted when a new output root is submitted.
        event OutputRootSubmitted(
            bytes32 indexed l2PostRoot,
            uint64 indexed l2BlockNumber,
            bytes32 l1Head
        );
    }

    /// Dispute game contract for challenging and resolving output roots.
    interface IOpenZkDisputeGame {
        /// Challenge an output root for a specific block number.
        function challenge(uint64 blockNumber) external payable;

        /// Resolve a dispute by submitting a valid proof.
        function resolve(
            bytes32 l1Head,
            bytes32 l2PreRoot,
            bytes32 l2PostRoot,
            uint64 l2BlockNumber,
            bytes32 rollupConfigHash,
            bytes32 programId,
            bytes calldata proof
        ) external;

        /// Check if a block's output root is currently disputed.
        function isDisputed(uint64 blockNumber) external view returns (bool);

        /// Emitted when a dispute is created.
        event DisputeCreated(uint64 indexed blockNumber, address indexed challenger);

        /// Emitted when a dispute is resolved.
        event DisputeResolved(uint64 indexed blockNumber, bool outputValid);
    }
}
