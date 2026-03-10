//! RPC-based proof submitter that calls the on-chain OpenZkL2OutputOracle.

use alloy_primitives::{Address, B256};
use alloy_provider::ProviderBuilder;
use async_trait::async_trait;
use open_zk_core::types::{ProofArtifact, StateTransitionJournal, ZkvmBackend};

use crate::abi::IOpenZkL2OutputOracle;
use crate::client::ProofSubmitter;

/// Submits proofs to the deployed OpenZkL2OutputOracle via RPC.
pub struct RpcProofSubmitter {
    rpc_url: String,
    oracle_address: Address,
    dispute_address: Option<Address>,
    private_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RpcSubmitterError {
    #[error("contract call failed: {0}")]
    ContractCall(String),
    #[error("unsupported backend: {0:?}")]
    UnsupportedBackend(ZkvmBackend),
    #[error("dispute game address not configured")]
    DisputeAddressNotConfigured,
}

impl RpcProofSubmitter {
    pub fn new(rpc_url: String, oracle_address: Address, private_key: String) -> Self {
        Self {
            rpc_url,
            oracle_address,
            dispute_address: None,
            private_key,
        }
    }

    /// Set the DisputeGame contract address for dispute resolution.
    pub fn with_dispute_address(mut self, address: Address) -> Self {
        self.dispute_address = Some(address);
        self
    }

    fn build_provider(&self) -> Result<impl alloy_provider::Provider + Clone, RpcSubmitterError> {
        let url: url::Url = self
            .rpc_url
            .parse()
            .map_err(|e| RpcSubmitterError::ContractCall(format!("invalid url: {e}")))?;

        let signer: alloy_provider::network::EthereumWallet = {
            let pk: alloy_signer_local::PrivateKeySigner = self
                .private_key
                .parse()
                .map_err(|e| RpcSubmitterError::ContractCall(format!("invalid key: {e}")))?;
            alloy_provider::network::EthereumWallet::from(pk)
        };

        let provider = ProviderBuilder::new().wallet(signer).connect_http(url);

        Ok(provider)
    }
}

#[async_trait]
impl ProofSubmitter for RpcProofSubmitter {
    type Error = RpcSubmitterError;

    async fn submit_proof(
        &self,
        journal: &StateTransitionJournal,
        proof: &ProofArtifact,
    ) -> Result<B256, Self::Error> {
        let provider = self.build_provider()?;
        let oracle = IOpenZkL2OutputOracle::new(self.oracle_address, &provider);
        let public_values = journal.to_abi_bytes();

        let tx_hash = match proof.backend {
            ZkvmBackend::Sp1 => {
                let pending = oracle
                    .submitSp1Proof(public_values.into(), proof.proof_bytes.clone().into())
                    .send()
                    .await
                    .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;
                tracing::info!("SP1 proof submitted, waiting for confirmation...");
                let receipt = pending
                    .get_receipt()
                    .await
                    .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;
                receipt.transaction_hash
            }
            ZkvmBackend::RiscZero => {
                let pending = oracle
                    .submitRisc0Proof(public_values.into(), proof.proof_bytes.clone().into())
                    .send()
                    .await
                    .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;
                tracing::info!("RISC Zero proof submitted, waiting for confirmation...");
                let receipt = pending
                    .get_receipt()
                    .await
                    .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;
                receipt.transaction_hash
            }
            other => return Err(RpcSubmitterError::UnsupportedBackend(other)),
        };

        Ok(tx_hash)
    }

    async fn resolve_dispute(
        &self,
        journal: &StateTransitionJournal,
        proof: &ProofArtifact,
    ) -> Result<B256, Self::Error> {
        let dispute_address = self
            .dispute_address
            .ok_or(RpcSubmitterError::DisputeAddressNotConfigured)?;

        let provider = self.build_provider()?;
        let dispute = crate::abi::IOpenZkDisputeGame::new(dispute_address, &provider);
        let public_values = journal.to_abi_bytes();

        let backend: u8 = match proof.backend {
            ZkvmBackend::Sp1 => 0,
            ZkvmBackend::RiscZero => 1,
            other => return Err(RpcSubmitterError::UnsupportedBackend(other)),
        };

        let pending = dispute
            .resolve(
                journal.l2_block_number,
                public_values.into(),
                proof.proof_bytes.clone().into(),
                backend,
            )
            .send()
            .await
            .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;

        tracing::info!("dispute resolution submitted, waiting for confirmation...");
        let receipt = pending
            .get_receipt()
            .await
            .map_err(|e| RpcSubmitterError::ContractCall(e.to_string()))?;

        Ok(receipt.transaction_hash)
    }
}
