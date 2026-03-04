use async_trait::async_trait;
use open_zk_core::types::{ProofArtifact, ProofRequest};
use std::fmt;

/// Status of a submitted proof job.
#[derive(Debug, Clone)]
pub enum ProofJobStatus {
    /// Job is queued and waiting to be processed.
    Queued,
    /// Job is currently being processed.
    InProgress,
    /// Job completed successfully with the resulting proof.
    Completed(ProofArtifact),
    /// Job failed with an error message.
    Failed(String),
}

/// Handle to a submitted proof job.
#[derive(Debug, Clone)]
pub struct ProofJobHandle {
    /// Unique identifier for this job.
    pub id: String,
    /// The original proof request.
    pub request: ProofRequest,
}

/// Dispatches proof generation jobs to a proving backend.
#[async_trait]
pub trait ProofDispatcher: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Submit a single range proof request.
    async fn submit(&self, request: ProofRequest) -> Result<ProofJobHandle, Self::Error>;

    /// Check the status of a submitted job.
    async fn status(&self, handle: &ProofJobHandle) -> Result<ProofJobStatus, Self::Error>;

    /// Wait for a job to complete and return the proof artifact.
    async fn wait(&self, handle: &ProofJobHandle) -> Result<ProofArtifact, Self::Error>;

    /// Submit an aggregation job over multiple range proofs.
    async fn submit_aggregation(
        &self,
        range_proofs: Vec<ProofArtifact>,
    ) -> Result<ProofJobHandle, Self::Error>;
}

/// A mock dispatcher that returns dummy proofs immediately.
#[derive(Debug, Default)]
pub struct MockDispatcher;

/// Error type for the mock dispatcher.
#[derive(Debug)]
pub struct MockDispatcherError(pub String);

impl fmt::Display for MockDispatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mock dispatcher error: {}", self.0)
    }
}

impl std::error::Error for MockDispatcherError {}

#[async_trait]
impl ProofDispatcher for MockDispatcher {
    type Error = MockDispatcherError;

    async fn submit(&self, request: ProofRequest) -> Result<ProofJobHandle, Self::Error> {
        let id = format!("mock-{}-{}", request.l2_start_block, request.l2_end_block);
        Ok(ProofJobHandle { id, request })
    }

    async fn status(&self, _handle: &ProofJobHandle) -> Result<ProofJobStatus, Self::Error> {
        Ok(ProofJobStatus::Completed(mock_proof_artifact()))
    }

    async fn wait(&self, _handle: &ProofJobHandle) -> Result<ProofArtifact, Self::Error> {
        Ok(mock_proof_artifact())
    }

    async fn submit_aggregation(
        &self,
        _range_proofs: Vec<ProofArtifact>,
    ) -> Result<ProofJobHandle, Self::Error> {
        let request = ProofRequest {
            l1_head: alloy_primitives::B256::ZERO,
            l2_start_block: 0,
            l2_end_block: 0,
            l2_start_output_root: alloy_primitives::B256::ZERO,
            mode: open_zk_core::types::ProvingMode::Groth16,
        };
        Ok(ProofJobHandle {
            id: "mock-aggregation".to_string(),
            request,
        })
    }
}

fn mock_proof_artifact() -> ProofArtifact {
    ProofArtifact {
        backend: open_zk_core::types::ZkvmBackend::Mock,
        mode: open_zk_core::types::ProvingMode::Groth16,
        proof_bytes: vec![0xDE, 0xAD],
        public_values: vec![],
        program_id: alloy_primitives::B256::ZERO,
        cycle_count: Some(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_dispatcher_submit_and_wait() {
        let dispatcher = MockDispatcher;
        let request = ProofRequest {
            l1_head: alloy_primitives::B256::ZERO,
            l2_start_block: 100,
            l2_end_block: 200,
            l2_start_output_root: alloy_primitives::B256::ZERO,
            mode: open_zk_core::types::ProvingMode::Groth16,
        };

        let handle = dispatcher.submit(request).await.unwrap();
        assert_eq!(handle.id, "mock-100-200");

        let proof = dispatcher.wait(&handle).await.unwrap();
        assert_eq!(proof.backend, open_zk_core::types::ZkvmBackend::Mock);
    }

    #[tokio::test]
    async fn mock_dispatcher_status_is_completed() {
        let dispatcher = MockDispatcher;
        let request = ProofRequest {
            l1_head: alloy_primitives::B256::ZERO,
            l2_start_block: 1,
            l2_end_block: 10,
            l2_start_output_root: alloy_primitives::B256::ZERO,
            mode: open_zk_core::types::ProvingMode::Groth16,
        };

        let handle = dispatcher.submit(request).await.unwrap();
        let status = dispatcher.status(&handle).await.unwrap();
        assert!(matches!(status, ProofJobStatus::Completed(_)));
    }

    #[tokio::test]
    async fn mock_dispatcher_aggregation() {
        let dispatcher = MockDispatcher;
        let proofs = vec![mock_proof_artifact(), mock_proof_artifact()];
        let handle = dispatcher.submit_aggregation(proofs).await.unwrap();
        assert_eq!(handle.id, "mock-aggregation");

        let proof = dispatcher.wait(&handle).await.unwrap();
        assert_eq!(proof.backend, open_zk_core::types::ZkvmBackend::Mock);
    }
}
