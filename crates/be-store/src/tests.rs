use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use uuid::Uuid;

use super::*;

mod a_different_key_produces_different_objects;
mod a_file_store_persists_objects_across_reopening;
mod a_manifest_round_trips_through_the_vault;
mod a_ranged_read_only_fetches_overlapping_chunks;
mod chunking_is_content_defined;
mod identical_content_deduplicates_across_writes;
mod tampered_bytes_fail_to_open;
mod values_round_trip_as_objects;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7374_2d63_6f6e_7465_6e74_2d30_3031);

fn key(seed: u8) -> ContentKey {
    ContentKey::from_bytes([seed; 32])
}

fn vault(seed: u8) -> Vault<MemoryStore> {
    Vault::new(MemoryStore::new(), key(seed)).with_chunker(ChunkerConfig::SMALL)
}

fn pseudorandom(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    (0..length)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            u8::try_from((state >> 33) & 0xff).unwrap()
        })
        .collect()
}

#[derive(Clone, Default)]
struct CountingStore {
    inner: MemoryStore,
    reads: Arc<AtomicUsize>,
    fetched: Arc<Mutex<Vec<Hash>>>,
}

impl CountingStore {
    fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }
}

impl ObjectStore for CountingStore {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        self.inner.put(bytes)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.fetched.lock().unwrap().push(hash);
        self.inner.get(hash)
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        self.inner.has(hash)
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        self.inner.remove(hash)
    }
}
