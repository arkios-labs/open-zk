//! Preimage key-value store for witness collection and serialization.
//!
//! Provides an `Arc<Mutex<HashMap>>`-backed `KeyValueStore` implementation
//! that allows retaining access to collected preimages after kona-host's
//! witness collection run completes.

#[cfg(feature = "kona")]
mod inner {
    use alloy_primitives::B256;
    use kona_host::KeyValueStore;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// A key-value store backed by `Arc<Mutex<HashMap>>`, allowing the caller
    /// to retain a handle to the underlying map after it is consumed by
    /// kona-host's `SplitKeyValueStore`.
    #[derive(Clone, Debug)]
    pub struct ArcMemoryKvStore {
        store: Arc<Mutex<HashMap<B256, Vec<u8>>>>,
    }

    impl ArcMemoryKvStore {
        /// Create a new empty store.
        pub fn new() -> Self {
            Self {
                store: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        /// Get a snapshot of all collected preimages.
        pub fn snapshot(&self) -> HashMap<B256, Vec<u8>> {
            self.store.lock().expect("poisoned lock").clone()
        }

        /// Number of entries in the store.
        pub fn len(&self) -> usize {
            self.store.lock().expect("poisoned lock").len()
        }

        /// Whether the store is empty.
        pub fn is_empty(&self) -> bool {
            self.len() == 0
        }
    }

    impl Default for ArcMemoryKvStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl KeyValueStore for ArcMemoryKvStore {
        fn get(&self, key: B256) -> Option<Vec<u8>> {
            self.store.lock().expect("poisoned lock").get(&key).cloned()
        }

        fn set(&mut self, key: B256, value: Vec<u8>) -> anyhow::Result<()> {
            self.store.lock().expect("poisoned lock").insert(key, value);
            Ok(())
        }
    }

    /// Serialize a preimage map to bytes.
    ///
    /// Wire format: repeated `[key: 32 bytes][value_len: 4 bytes LE][value: N bytes]`.
    pub fn serialize_preimages(preimages: &HashMap<B256, Vec<u8>>) -> Vec<u8> {
        let mut buf = Vec::new();
        // Write entry count first for deterministic deserialization
        buf.extend_from_slice(&(preimages.len() as u32).to_le_bytes());
        for (key, value) in preimages {
            buf.extend_from_slice(key.as_slice());
            buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
            buf.extend_from_slice(value);
        }
        buf
    }

    /// Deserialize preimages from bytes produced by [`serialize_preimages`].
    pub fn deserialize_preimages(data: &[u8]) -> Option<HashMap<B256, Vec<u8>>> {
        let mut offset = 0;

        if offset + 4 > data.len() {
            return None;
        }
        let count =
            u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;

        let mut map = HashMap::with_capacity(count);
        for _ in 0..count {
            if offset + 32 > data.len() {
                return None;
            }
            let key = B256::from_slice(&data[offset..offset + 32]);
            offset += 32;

            if offset + 4 > data.len() {
                return None;
            }
            let value_len =
                u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
            offset += 4;

            if offset + value_len > data.len() {
                return None;
            }
            let value = data[offset..offset + value_len].to_vec();
            offset += value_len;

            map.insert(key, value);
        }

        Some(map)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn arc_memory_kv_store_basic() {
            let mut store = ArcMemoryKvStore::new();
            let key = B256::repeat_byte(0x01);
            let value = vec![0xDE, 0xAD];

            assert!(store.get(key).is_none());
            store.set(key, value.clone()).unwrap();
            assert_eq!(store.get(key).unwrap(), value);
            assert_eq!(store.len(), 1);
        }

        #[test]
        fn arc_memory_kv_store_clone_shares_data() {
            let mut store = ArcMemoryKvStore::new();
            let cloned = store.clone();

            let key = B256::repeat_byte(0x02);
            store.set(key, vec![1, 2, 3]).unwrap();

            // The clone sees the same data (shared Arc)
            assert_eq!(cloned.get(key).unwrap(), vec![1, 2, 3]);
        }

        #[test]
        fn arc_memory_kv_store_snapshot() {
            let mut store = ArcMemoryKvStore::new();
            store
                .set(B256::repeat_byte(0x01), vec![0xAA])
                .unwrap();
            store
                .set(B256::repeat_byte(0x02), vec![0xBB])
                .unwrap();

            let snap = store.snapshot();
            assert_eq!(snap.len(), 2);
            assert_eq!(snap[&B256::repeat_byte(0x01)], vec![0xAA]);
            assert_eq!(snap[&B256::repeat_byte(0x02)], vec![0xBB]);
        }

        #[test]
        fn serialize_deserialize_roundtrip() {
            let mut map = HashMap::new();
            map.insert(B256::repeat_byte(0x01), vec![0xDE, 0xAD]);
            map.insert(B256::repeat_byte(0x02), vec![0xBE, 0xEF, 0xCA, 0xFE]);
            map.insert(B256::repeat_byte(0x03), vec![]);

            let serialized = serialize_preimages(&map);
            let deserialized = deserialize_preimages(&serialized).unwrap();

            assert_eq!(map, deserialized);
        }

        #[test]
        fn serialize_deserialize_empty() {
            let map = HashMap::new();
            let serialized = serialize_preimages(&map);
            let deserialized = deserialize_preimages(&serialized).unwrap();
            assert!(deserialized.is_empty());
        }

        #[test]
        fn deserialize_truncated_returns_none() {
            assert!(deserialize_preimages(&[]).is_none());
            // Count says 1 entry but no data follows
            assert!(deserialize_preimages(&[1, 0, 0, 0]).is_none());
        }

        #[test]
        fn serialize_large_values() {
            let mut map = HashMap::new();
            let big_value = vec![0x42; 100_000];
            map.insert(B256::repeat_byte(0xFF), big_value.clone());

            let serialized = serialize_preimages(&map);
            let deserialized = deserialize_preimages(&serialized).unwrap();
            assert_eq!(deserialized[&B256::repeat_byte(0xFF)], big_value);
        }
    }
}

#[cfg(feature = "kona")]
pub use inner::*;
