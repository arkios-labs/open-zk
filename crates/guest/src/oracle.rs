//! Preimage store for zkVM guest programs.
//!
//! Stores the preimage KV data from the host witness and serves lookups
//! to kona's derivation pipeline. All data is read-only after construction.
//!
//! Deserialized from rkyv-encoded bytes via `from_rkyv_bytes()`.

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use async_trait::async_trait;
use kona_preimage::errors::{PreimageOracleError, PreimageOracleResult};
use kona_preimage::{HintWriterClient, PreimageKey, PreimageOracleClient};
use rkyv::Deserialize as _;

/// Preimage store backed by host-provided data.
///
/// Implements [`PreimageOracleClient`] and [`HintWriterClient`] (no-op),
/// satisfying kona's [`CommsClient`](kona_preimage::CommsClient) super-trait
/// when `Clone` is also implemented.
///
/// Internally wraps data in `Arc` so clones are cheap.
#[derive(Clone, Debug)]
pub struct PreimageStore {
    data: Arc<BTreeMap<[u8; 32], Vec<u8>>>,
}

impl PreimageStore {
    /// Create from an already-deserialized map.
    pub fn new(data: BTreeMap<[u8; 32], Vec<u8>>) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    /// Deserialize from rkyv-encoded bytes.
    ///
    /// The host serializes a `BTreeMap<[u8; 32], Vec<u8>>` with `rkyv::to_bytes()`.
    /// Uses `AlignedVec` to ensure proper alignment for rkyv's archived root.
    pub fn from_rkyv_bytes(raw: &[u8]) -> Option<Self> {
        let mut aligned = rkyv::AlignedVec::with_capacity(raw.len());
        aligned.extend_from_slice(raw);
        let archived = rkyv::check_archived_root::<BTreeMap<[u8; 32], Vec<u8>>>(&aligned).ok()?;
        let data: BTreeMap<[u8; 32], Vec<u8>> = archived.deserialize(&mut rkyv::Infallible).ok()?;
        Some(Self {
            data: Arc::new(data),
        })
    }

    /// Number of preimage entries.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the store has no entries.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[async_trait]
impl PreimageOracleClient for PreimageStore {
    async fn get(&self, key: PreimageKey) -> PreimageOracleResult<Vec<u8>> {
        let key_bytes: [u8; 32] = key.into();
        self.data
            .get(&key_bytes)
            .cloned()
            .ok_or(PreimageOracleError::KeyNotFound)
    }

    async fn get_exact(&self, key: PreimageKey, buf: &mut [u8]) -> PreimageOracleResult<()> {
        let value = self.get(key).await?;
        if buf.len() != value.len() {
            return Err(PreimageOracleError::BufferLengthMismatch(
                value.len(),
                buf.len(),
            ));
        }
        buf.copy_from_slice(&value);
        Ok(())
    }
}

#[async_trait]
impl HintWriterClient for PreimageStore {
    async fn write(&self, _hint: &str) -> PreimageOracleResult<()> {
        // No-op: all preimages are pre-fetched by the host.
        Ok(())
    }
}
