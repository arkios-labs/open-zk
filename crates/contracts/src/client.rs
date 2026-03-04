//! On-chain proof submission trait and mock implementation.

use alloy_primitives::B256;
use async_trait::async_trait;
use open_zk_core::types::{ProofArtifact, StateTransitionJournal};

/// Submits proofs to on-chain contracts.
#[async_trait]
pub trait ProofSubmitter: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Submit a state transition proof to the L2 Output Oracle.
    ///
    /// Returns the transaction hash.
    async fn submit_proof(
        &self,
        journal: &StateTransitionJournal,
        proof: &ProofArtifact,
    ) -> Result<B256, Self::Error>;

    /// Resolve a dispute by submitting a valid proof.
    ///
    /// Returns the transaction hash.
    async fn resolve_dispute(
        &self,
        journal: &StateTransitionJournal,
        proof: &ProofArtifact,
    ) -> Result<B256, Self::Error>;
}

/// Mock proof submitter for testing. Returns zero tx hashes.
#[derive(Debug, Default)]
pub struct MockProofSubmitter;

#[derive(Debug, thiserror::Error)]
#[error("mock submitter error: {0}")]
pub struct MockSubmitterError(pub String);

#[async_trait]
impl ProofSubmitter for MockProofSubmitter {
    type Error = MockSubmitterError;

    async fn submit_proof(
        &self,
        _journal: &StateTransitionJournal,
        _proof: &ProofArtifact,
    ) -> Result<B256, Self::Error> {
        Ok(B256::ZERO)
    }

    async fn resolve_dispute(
        &self,
        _journal: &StateTransitionJournal,
        _proof: &ProofArtifact,
    ) -> Result<B256, Self::Error> {
        Ok(B256::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_zk_core::types::{ProvingMode, ZkvmBackend};

    fn sample_journal() -> StateTransitionJournal {
        StateTransitionJournal {
            l1_head: B256::repeat_byte(0x01),
            l2_pre_root: B256::repeat_byte(0x02),
            l2_post_root: B256::repeat_byte(0x03),
            l2_block_number: 100,
            rollup_config_hash: B256::repeat_byte(0x04),
            program_id: B256::repeat_byte(0x05),
        }
    }

    fn sample_proof() -> ProofArtifact {
        ProofArtifact {
            backend: ZkvmBackend::Mock,
            mode: ProvingMode::Groth16,
            proof_bytes: vec![0xDE, 0xAD],
            public_values: vec![],
            program_id: B256::ZERO,
            cycle_count: Some(0),
        }
    }

    #[tokio::test]
    async fn mock_submitter_returns_zero_tx_hash() {
        let submitter = MockProofSubmitter;
        let tx_hash = submitter
            .submit_proof(&sample_journal(), &sample_proof())
            .await
            .unwrap();
        assert_eq!(tx_hash, B256::ZERO);
    }

    #[tokio::test]
    async fn mock_submitter_resolve_dispute() {
        let submitter = MockProofSubmitter;
        let tx_hash = submitter
            .resolve_dispute(&sample_journal(), &sample_proof())
            .await
            .unwrap();
        assert_eq!(tx_hash, B256::ZERO);
    }
}
