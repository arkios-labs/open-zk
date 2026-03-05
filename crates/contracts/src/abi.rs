//! Solidity ABI definitions for on-chain verifier and oracle contracts.

use alloy_sol_types::sol;

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
}
