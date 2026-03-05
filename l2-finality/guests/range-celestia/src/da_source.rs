//! Celestia data availability source for the OP Stack derivation pipeline.
//!
//! Reads L1 batch inbox transactions containing Celestia DA commitments,
//! resolves them to actual batch data via the preimage oracle, and feeds
//! the resolved data to the derivation pipeline.
//!
//! # Celestia Commitment Format
//!
//! OP Stack Alt-DA commitments in L1 calldata:
//! - Byte 0: derivation version (0x00 = generic commitment, 0xce = Celestia)
//! - Remaining bytes: DA-specific commitment data
//!
//! For Celestia, the commitment encodes the namespace, block height,
//! and share range needed to retrieve the blob data.
//!
//! # Preimage Resolution
//!
//! The host pre-fetches all Celestia blob data and stores it in the
//! preimage oracle keyed by `keccak256(commitment)`. The guest resolves
//! commitments by computing the same key and looking up the preimage.

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloy_consensus::transaction::SignerRecoverable;
use alloy_consensus::{Transaction, TxEnvelope};
use alloy_primitives::{Address, Bytes, keccak256};
use async_trait::async_trait;
use kona_derive::{ChainProvider, DataAvailabilityProvider, PipelineError, PipelineResult};
use kona_preimage::{PreimageKey, PreimageKeyType, PreimageOracleClient};
use kona_protocol::BlockInfo;

/// Celestia derivation version byte in OP Stack Alt-DA commitments.
///
/// When the first byte of batch inbox calldata is this value, the remaining
/// bytes are a Celestia-specific commitment that must be resolved via the
/// oracle to obtain the actual batch data.
const CELESTIA_COMMITMENT_PREFIX: u8 = 0xce;

/// Generic commitment prefix (used by some OP Alt-DA implementations).
const GENERIC_COMMITMENT_PREFIX: u8 = 0x00;

/// Data availability source that resolves Celestia commitments from L1
/// batch inbox transactions via the preimage oracle.
#[derive(Debug, Clone)]
pub struct CelestiaDataSource<CP, O>
where
    CP: ChainProvider + Send,
    O: PreimageOracleClient + Send + Sync,
{
    /// L1 chain provider for reading block transactions.
    chain_provider: CP,
    /// Preimage oracle for resolving commitments to batch data.
    oracle: Arc<O>,
    /// Batch inbox address on L1.
    batch_inbox_address: Address,
    /// Resolved batch data for the current L1 block.
    batch_data: VecDeque<Bytes>,
    /// Whether data has been loaded for the current block.
    open: bool,
}

impl<CP, O> CelestiaDataSource<CP, O>
where
    CP: ChainProvider + Send,
    O: PreimageOracleClient + Send + Sync,
{
    /// Creates a new Celestia data availability source.
    pub fn new(chain_provider: CP, oracle: Arc<O>, batch_inbox_address: Address) -> Self {
        Self {
            chain_provider,
            oracle,
            batch_inbox_address,
            batch_data: VecDeque::new(),
            open: false,
        }
    }

    /// Loads and resolves batch data from L1 block transactions.
    ///
    /// For each batch inbox transaction from the batcher:
    /// 1. Check if calldata contains a Celestia commitment (prefix byte)
    /// 2. If yes: resolve the commitment via oracle to get actual batch data
    /// 3. If no: treat as regular calldata (fallback for mixed DA modes)
    async fn load_batch_data(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> Result<(), CP::Error> {
        if self.open {
            return Ok(());
        }

        let (_, txs) = self
            .chain_provider
            .block_info_and_transactions_by_hash(block_ref.hash)
            .await?;

        for tx in txs.iter() {
            let (tx_kind, data) = match tx {
                TxEnvelope::Legacy(tx) => (tx.tx().to(), tx.tx().input()),
                TxEnvelope::Eip2930(tx) => (tx.tx().to(), tx.tx().input()),
                TxEnvelope::Eip1559(tx) => (tx.tx().to(), tx.tx().input()),
                _ => continue,
            };

            // Filter: must be sent to batch inbox by the batcher
            let Some(to) = tx_kind else { continue };
            if to != self.batch_inbox_address {
                continue;
            }
            if tx.recover_signer().ok() != Some(batcher_address) {
                continue;
            }

            if data.is_empty() {
                continue;
            }

            // Check the commitment prefix byte
            match data[0] {
                CELESTIA_COMMITMENT_PREFIX | GENERIC_COMMITMENT_PREFIX => {
                    // Resolve the Celestia commitment via oracle
                    let commitment = &data[1..];
                    if let Ok(resolved) = self.resolve_commitment(commitment).await {
                        self.batch_data.push_back(resolved);
                    }
                }
                _ => {
                    // Regular calldata (no commitment prefix) — pass through
                    self.batch_data.push_back(data.to_vec().into());
                }
            }
        }

        self.open = true;
        Ok(())
    }

    /// Resolves a Celestia commitment to actual batch data via the preimage oracle.
    ///
    /// The host stores Celestia blob data under `keccak256(commitment)`.
    /// The guest computes the same key to look up the data.
    async fn resolve_commitment(&self, commitment: &[u8]) -> Result<Bytes, ()> {
        let commitment_hash = keccak256(commitment);
        let key = PreimageKey::new(*commitment_hash, PreimageKeyType::Keccak256);
        self.oracle.get(key).await.map(|data| data.into()).map_err(|_| ())
    }
}

#[async_trait]
impl<CP, O> DataAvailabilityProvider for CelestiaDataSource<CP, O>
where
    CP: ChainProvider + Send + Sync + Clone + core::fmt::Debug,
    O: PreimageOracleClient + Send + Sync + core::fmt::Debug + Clone,
{
    type Item = Bytes;

    async fn next(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> PipelineResult<Self::Item> {
        self.load_batch_data(block_ref, batcher_address)
            .await
            .map_err(Into::into)?;
        self.batch_data.pop_front().ok_or(PipelineError::Eof.temp())
    }

    fn clear(&mut self) {
        self.batch_data.clear();
        self.open = false;
    }
}
