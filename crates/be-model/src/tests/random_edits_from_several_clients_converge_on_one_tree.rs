use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn random_edits_from_several_clients_converge_on_one_tree() {
    for seed in 1..=100u64 {
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let len = 64 + (next() % 3072) as usize;
        let data: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        if catch_unwind(AssertUnwindSafe(|| crate::fuzz::lists(&data))).is_err() {
            panic!("seed {seed} failed; input {data:?}");
        }
    }
}
