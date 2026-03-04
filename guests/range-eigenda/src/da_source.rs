//! EigenDA data availability source for the OP Stack derivation pipeline.
//!
//! Reads L1 batch inbox transactions containing EigenDA commitments,
//! resolves them to actual batch data via the preimage oracle, and feeds
//! the resolved data to the derivation pipeline.
//!
//! # EigenDA Commitment Format
//!
//! OP Stack Alt-DA commitments in L1 calldata:
//! - Byte 0: derivation version (0x00 = generic, 0xed = EigenDA-specific)
//! - Remaining bytes: DA-specific commitment data
//!
//! For EigenDA, the commitment encodes the batch header hash, blob index,
//! and KZG commitment needed to retrieve and verify the blob data.
//!
//! # Preimage Resolution
//!
//! The host pre-fetches all EigenDA blob data and stores it in the
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

/// EigenDA-specific derivation version byte in OP Stack Alt-DA commitments.
const EIGENDA_COMMITMENT_PREFIX: u8 = 0xed;

/// Generic commitment prefix (used by some OP Alt-DA implementations).
const GENERIC_COMMITMENT_PREFIX: u8 = 0x00;

/// Data availability source that resolves EigenDA commitments from L1
/// batch inbox transactions via the preimage oracle.
///
/// EigenDA stores rollup batch data as blobs in its dispersal network.
/// The batcher posts commitments (blob header hash + KZG info) to L1,
/// and the host pre-fetches the actual data for the guest to verify.
#[derive(Debug, Clone)]
pub struct EigenDADataSource<CP, O>
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

impl<CP, O> EigenDADataSource<CP, O>
where
    CP: ChainProvider + Send,
    O: PreimageOracleClient + Send + Sync,
{
    /// Creates a new EigenDA data availability source.
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
    /// 1. Check if calldata contains an EigenDA commitment (prefix byte)
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

            match data[0] {
                EIGENDA_COMMITMENT_PREFIX | GENERIC_COMMITMENT_PREFIX => {
                    // Resolve the EigenDA commitment via oracle
                    let commitment = &data[1..];
                    if let Ok(resolved) = self.resolve_commitment(commitment).await {
                        self.batch_data.push_back(resolved);
                    }
                }
                _ => {
                    // Regular calldata — pass through
                    self.batch_data.push_back(data.to_vec().into());
                }
            }
        }

        self.open = true;
        Ok(())
    }

    /// Resolves an EigenDA commitment to actual batch data via the preimage oracle.
    ///
    /// The host stores EigenDA blob data under `keccak256(commitment)`.
    /// The guest computes the same key to look up the data.
    async fn resolve_commitment(&self, commitment: &[u8]) -> Result<Bytes, ()> {
        let commitment_hash = keccak256(commitment);
        let key = PreimageKey::new(*commitment_hash, PreimageKeyType::Keccak256);
        self.oracle.get(key).await.map(|data| data.into()).map_err(|_| ())
    }
}

#[async_trait]
impl<CP, O> DataAvailabilityProvider for EigenDADataSource<CP, O>
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
