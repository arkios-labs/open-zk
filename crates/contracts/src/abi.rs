//! Solidity ABI definitions for on-chain verifier and oracle contracts.

use alloy_sol_types::sol;

#[cfg(feature = "rpc")]
sol! {
    /// OpenZkL2OutputOracle — matches the deployed Solidity contract.
    #[sol(rpc)]
    interface IOpenZkL2OutputOracle {
        /// Submit an SP1-proven state transition.
        function submitSp1Proof(
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external;

        /// Submit a RISC Zero-proven state transition.
        function submitRisc0Proof(
            bytes calldata journalBytes,
            bytes calldata seal
        ) external;

        /// Get the latest proven L2 output root.
        function latestOutputRoot() external view returns (bytes32);

        /// Get the latest proven L2 block number.
        function latestBlockNumber() external view returns (uint64);

        /// Check if a specific block has been proven.
        function isBlockProven(uint64 blockNumber) external view returns (bool);

        /// Update the SP1 program verification key (owner only).
        function setSp1ProgramVKey(bytes32 _vkey) external;

        /// Update the RISC Zero image ID (owner only).
        function setRisc0ImageId(bytes32 _imageId) external;
    }

    /// OpenZkDisputeGame — matches the deployed Solidity contract.
    #[sol(rpc)]
    interface IOpenZkDisputeGame {
        /// Challenge a proven output root.
        function challenge(uint64 blockNumber) external;

        /// Resolve a dispute with a ZK proof.
        /// backend: 0 = SP1, 1 = RiscZero.
        function resolve(
            uint64 blockNumber,
            bytes calldata publicValues,
            bytes calldata proofBytes,
            uint8 backend
        ) external;

        /// Check if a block is currently under dispute.
        function isDisputed(uint64 blockNumber) external view returns (bool);
    }
}

#[cfg(not(feature = "rpc"))]
sol! {
    /// OpenZkL2OutputOracle — matches the deployed Solidity contract.
    interface IOpenZkL2OutputOracle {
        /// Submit an SP1-proven state transition.
        function submitSp1Proof(
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external;

        /// Submit a RISC Zero-proven state transition.
        function submitRisc0Proof(
            bytes calldata journalBytes,
            bytes calldata seal
        ) external;

        /// Get the latest proven L2 output root.
        function latestOutputRoot() external view returns (bytes32);

        /// Get the latest proven L2 block number.
        function latestBlockNumber() external view returns (uint64);

        /// Check if a specific block has been proven.
        function isBlockProven(uint64 blockNumber) external view returns (bool);

        /// Update the SP1 program verification key (owner only).
        function setSp1ProgramVKey(bytes32 _vkey) external;

        /// Update the RISC Zero image ID (owner only).
        function setRisc0ImageId(bytes32 _imageId) external;
    }

    /// OpenZkDisputeGame — matches the deployed Solidity contract.
    interface IOpenZkDisputeGame {
        /// Challenge a proven output root.
        function challenge(uint64 blockNumber) external;

        /// Resolve a dispute with a ZK proof.
        /// backend: 0 = SP1, 1 = RiscZero.
        function resolve(
            uint64 blockNumber,
            bytes calldata publicValues,
            bytes calldata proofBytes,
            uint8 backend
        ) external;

        /// Check if a block is currently under dispute.
        function isDisputed(uint64 blockNumber) external view returns (bool);
    }
}
