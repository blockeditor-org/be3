use super::*;

/// A deterministic stream of bytes (xorshift), so the test is the same every run.
fn noise(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed | 1;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

#[test]
fn fuzzed_inputs_never_panic_and_built_values_read_back() {
    for seed in 0..2000u64 {
        let data = noise(seed, (seed % 300) as usize);
        crate::fuzz::round_trip(&data);
        crate::fuzz::decode_any(&data);
        // A valid encoding, cut short and with a byte flipped, is the input most likely to
        // reach deep into a decoder.
        let mut valid = encode(&crate::fuzz::FuzzRecord::built(&data));
        crate::fuzz::decode_any(&valid);
        if !valid.is_empty() {
            let at = (seed as usize * 7919) % valid.len();
            valid[at] ^= 1 << (seed % 8);
            crate::fuzz::decode_any(&valid);
            valid.truncate(at);
            crate::fuzz::decode_any(&valid);
        }
    }
}
