use std::panic::{AssertUnwindSafe, catch_unwind};

use super::*;

#[test]
fn random_bytes_drive_every_replica_to_the_reference() {
    for seed in 1..=100u64 {
        let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let len = 64 + random.below(3072);
        let data: Vec<u8> = (0..len).map(|_| random.next() as u8).collect();
        if catch_unwind(AssertUnwindSafe(|| super::super::fuzz::sequence(&data))).is_err() {
            panic!("seed {seed} failed; input {data:?}");
        }
    }
}
