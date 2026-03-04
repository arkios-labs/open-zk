use async_trait::async_trait;
use open_zk_core::traits::{RawWitness, WitnessProvider};
use open_zk_core::types::ProofRequest;

/// A mock witness provider that returns deterministic dummy data
/// derived from the [`ProofRequest`] fields.
///
/// Useful for integration tests that need to exercise the full pipeline
/// without real RPC connections or L1/L2 data.
#[derive(Debug, Default)]
pub struct MockWitnessProvider;

#[derive(Debug, thiserror::Error)]
#[error("mock witness error: {0}")]
pub struct MockWitnessError(pub String);

#[async_trait]
impl WitnessProvider for MockWitnessProvider {
    type Error = MockWitnessError;

    async fn generate_witness(&self, request: &ProofRequest) -> Result<RawWitness, Self::Error> {
        // Generate deterministic boot_info from the request's block range
        let boot_info = format!(
            "boot:{}:{}:{}",
            request.l2_start_block, request.l2_end_block, request.l1_head
        )
        .into_bytes();

        // Deterministic oracle data keyed on block range
        let oracle_data = format!(
            "oracle:{}:{}",
            request.l2_start_block, request.l2_end_block
        )
        .into_bytes();

        // Deterministic blob data
        let blob_data = format!("blob:{}", request.l1_head).into_bytes();

        Ok(RawWitness {
            boot_info,
            oracle_data,
            blob_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use open_zk_core::types::ProvingMode;

    #[tokio::test]
    async fn mock_witness_is_deterministic() {
        let provider = MockWitnessProvider;
        let request = ProofRequest {
            l1_head: B256::ZERO,
            l2_start_block: 100,
            l2_end_block: 200,
            l2_start_output_root: B256::ZERO,
            mode: ProvingMode::Execute,
        };

        let w1 = provider.generate_witness(&request).await.unwrap();
        let w2 = provider.generate_witness(&request).await.unwrap();

        assert_eq!(w1.boot_info, w2.boot_info);
        assert_eq!(w1.oracle_data, w2.oracle_data);
        assert_eq!(w1.blob_data, w2.blob_data);
        assert!(!w1.boot_info.is_empty());
    }
}
