//! Preimage store for zkVM guest programs.
//!
//! Stores the preimage KV data from the host witness and serves lookups
//! to kona's derivation pipeline. All data is read-only after construction.
//!
//! Supports two deserialization formats:
//! - **rkyv** (preferred): zero-copy friendly via `from_rkyv_bytes()`
//! - **raw** (legacy): custom wire format via `from_raw_bytes()`

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
    /// This is the preferred deserialization path. The host serializes
    /// a `BTreeMap<[u8; 32], Vec<u8>>` with `rkyv::to_bytes()`.
    pub fn from_rkyv_bytes(raw: &[u8]) -> Option<Self> {
        let archived = rkyv::check_archived_root::<BTreeMap<[u8; 32], Vec<u8>>>(raw).ok()?;
        let data: BTreeMap<[u8; 32], Vec<u8>> =
            archived.deserialize(&mut rkyv::Infallible).ok()?;
        Some(Self {
            data: Arc::new(data),
        })
    }

    /// Deserialize from raw bytes produced by the legacy `serialize_preimages()`.
    ///
    /// Wire format: `[count: 4 bytes LE]` repeated `[key: 32][value_len: 4 LE][value: N]`
    pub fn from_raw_bytes(raw: &[u8]) -> Option<Self> {
        let mut offset = 0;

        if offset + 4 > raw.len() {
            return None;
        }
        let count = u32::from_le_bytes(raw[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;

        let mut data = BTreeMap::new();
        for _ in 0..count {
            if offset + 32 > raw.len() {
                return None;
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw[offset..offset + 32]);
            offset += 32;

            if offset + 4 > raw.len() {
                return None;
            }
            let value_len =
                u32::from_le_bytes(raw[offset..offset + 4].try_into().ok()?) as usize;
            offset += 4;

            if offset + value_len > raw.len() {
                return None;
            }
            let value = raw[offset..offset + value_len].to_vec();
            offset += value_len;

            data.insert(key, value);
        }

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

/// Type alias for backward compatibility.
pub type InMemoryOracle = PreimageStore;
